use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use crate::error::Result;
use crate::services::cloud::CloudProvider;

// ─── Tree node ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileTreeNode {
    pub name:          String,
    /// Relative path from save root, using forward slashes
    pub relative_path: String,
    pub is_directory:  bool,
    pub size:          u64,
    /// Last modification time, Unix milliseconds (files only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified:      Option<i64>,
    pub children:      Vec<FileTreeNode>,
}

/// Parse a cloud timestamp (WebDAV RFC 2822 or RFC 3339) to Unix milliseconds
fn parse_cloud_time(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc2822(s)
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(s))
        .ok()
        .map(|d| d.timestamp_millis())
}

impl FileTreeNode {
    /// Total number of files (recursively, not directories)
    pub fn file_count(&self) -> usize {
        if !self.is_directory { return 1; }
        self.children.iter().map(|c| c.file_count()).sum()
    }

    /// Total size of all files (recursively)
    pub fn total_size(&self) -> u64 {
        if !self.is_directory { return self.size; }
        self.children.iter().map(|c| c.total_size()).sum()
    }
}

// ─── Local filesystem tree ────────────────────────────────────────────────────

/// Safety limits to guard against pathological filesystem structures
/// (deep nesting, symlink cycles, junction points, etc.)
const MAX_DEPTH: usize = 128;

pub fn build_local_tree(root: &Path) -> Result<Vec<FileTreeNode>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    Ok(read_dir_iterative(root))
}

/// Builds the tree for `root` iteratively (explicit stack instead of
/// recursive function calls), so pathological directory structures can
/// never overflow the call stack.
///
/// Also guards against symlink/junction cycles by tracking canonicalized
/// paths already visited, and against runaway depth via `MAX_DEPTH`.
fn read_dir_iterative(root: &Path) -> Vec<FileTreeNode> {
    // Work item: the directory to read, its depth, and a pointer to where
    // its resulting children should be written back to.
    enum WorkItem {
        Dir { path: PathBuf, depth: usize, node_path: Vec<usize> },
    }

    let mut visited: HashSet<PathBuf> = HashSet::new();
    if let Ok(canon) = std::fs::canonicalize(root) {
        visited.insert(canon);
    }

    let mut top_level: Vec<FileTreeNode> = Vec::new();
    let mut stack: Vec<WorkItem> = vec![WorkItem::Dir {
        path: root.to_path_buf(),
        depth: 0,
        node_path: vec![],
    }];

    while let Some(WorkItem::Dir { path, depth, node_path }) = stack.pop() {
        if depth >= MAX_DEPTH {
            // Directory is suspiciously deep (likely a cycle or a
            // pathological structure) - stop descending here.
            continue;
        }

        let Ok(entries) = std::fs::read_dir(&path) else { continue };

        let mut nodes: Vec<FileTreeNode> = Vec::new();
        let mut subdirs: Vec<(PathBuf, usize)> = Vec::new(); // (path, index in `nodes`)

        for entry in entries.filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let rel = entry_path
                .strip_prefix(root)
                .unwrap_or(&entry_path)
                .to_string_lossy()
                .replace('\\', "/");

            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };

            // Don't follow symlinks/junctions: list them as leaf entries
            // (size 0) so a cycle (e.g. a link pointing at an ancestor
            // directory) can never cause unbounded traversal.
            if file_type.is_symlink() {
                nodes.push(FileTreeNode {
                    name,
                    relative_path: rel,
                    is_directory: false,
                    size: 0,
                    modified: None,
                    children: vec![],
                });
                continue;
            }

            if file_type.is_dir() {
                // Cycle guard: skip directories we've already visited
                // (covers junction points / hard-linked directories that
                // `is_symlink()` alone wouldn't catch on Windows).
                let already_visited = match std::fs::canonicalize(&entry_path) {
                    Ok(canon) => !visited.insert(canon),
                    Err(_) => false,
                };

                let idx = nodes.len();
                nodes.push(FileTreeNode {
                    name,
                    relative_path: rel,
                    is_directory: true,
                    size: 0,
                    modified: None,
                    children: vec![],
                });

                if !already_visited {
                    subdirs.push((entry_path, idx));
                }
            } else {
                let meta = std::fs::metadata(&entry_path).ok();
                let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                let modified = meta.and_then(|m| m.modified().ok())
                    .map(|t| chrono::DateTime::<chrono::Utc>::from(t).timestamp_millis());
                nodes.push(FileTreeNode {
                    name,
                    relative_path: rel,
                    is_directory: false,
                    size,
                    modified,
                    children: vec![],
                });
            }
        }

        nodes.sort_by(|a, b| match (a.is_directory, b.is_directory) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        // Re-derive subdir indices after sort by matching on path segments
        // is fragile; instead, queue subdirs using their relative_path so
        // we can locate them post-sort without relying on pre-sort index.
        for (subdir_path, _old_idx) in &subdirs {
            let subdir_name = subdir_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Some(new_idx) = nodes
                .iter()
                .position(|n| n.is_directory && n.name == subdir_name)
            {
                let mut child_node_path = node_path.clone();
                child_node_path.push(new_idx);
                stack.push(WorkItem::Dir {
                    path: subdir_path.clone(),
                    depth: depth + 1,
                    node_path: child_node_path,
                });
            }
        }

        // Write `nodes` back into the tree at `node_path`.
        write_at_path(&mut top_level, &node_path, nodes);
    }

    top_level
}

/// Writes `nodes` as the `children` of the tree node located at `path`
/// (a sequence of child indices from the top level). An empty `path`
/// means `nodes` is the top-level list itself.
fn write_at_path(top_level: &mut Vec<FileTreeNode>, path: &[usize], nodes: Vec<FileTreeNode>) {
    if path.is_empty() {
        *top_level = nodes;
        return;
    }
    let mut current = top_level;
    for &idx in &path[..path.len() - 1] {
        current = &mut current[idx].children;
    }
    let last = *path.last().unwrap();
    current[last].children = nodes;
}

// ─── Cloud tree ───────────────────────────────────────────────────────────────
//
// NOTE: build_cloud_subtree remains recursive (via Pin<Box<Future>>) since
// it is bounded by the remote provider's own directory structure and API
// call depth, not by an unbounded local filesystem. If the cloud provider
// can also expose cyclical structures, the same iterative-with-visited-set
// approach should be applied here as well.

pub async fn build_cloud_tree(
    cloud: &dyn CloudProvider,
    cloud_saves_path: &str,
) -> Vec<FileTreeNode> {
    build_cloud_subtree(cloud, cloud_saves_path, cloud_saves_path).await
}

fn build_cloud_subtree<'a>(
    cloud: &'a dyn CloudProvider,
    path: &'a str,
    base: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<FileTreeNode>> + Send + 'a>> {
    Box::pin(async move {
        let Ok(items) = cloud.list_files(path).await else { return vec![]; };
        let mut nodes = Vec::new();

        // List all subdirectories concurrently
        let subtrees = futures::future::join_all(items.iter()
            .map(|item| async move {
                if item.is_directory { build_cloud_subtree(cloud, &item.path, base).await } else { vec![] }
            })).await;
        for (item, children) in items.iter().zip(subtrees) {
            let name = item.name.clone();
            let rel = if item.path.starts_with(base) {
                item.path[base.len()..].trim_start_matches('/').to_string()
            } else {
                item.path.rsplit('/').next().unwrap_or(&item.name).to_string()
            };

            if item.is_directory {
                nodes.push(FileTreeNode {
                    name, relative_path: rel, is_directory: true, size: 0, modified: None, children,
                });
            } else {
                nodes.push(FileTreeNode {
                    name, relative_path: rel, is_directory: false,
                    size: item.size,
                    modified: item.modified_at.as_deref().and_then(parse_cloud_time),
                    children: vec![],
                });
            }
        }

        nodes.sort_by(|a, b| {
            match (a.is_directory, b.is_directory) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });
        nodes
    })
}

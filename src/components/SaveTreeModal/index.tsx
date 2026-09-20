import React, { useState, useEffect, useCallback } from 'react';
import { useIpc } from '../../hooks/useIpc';
import { Icon } from '../common/Icon';
import type { FileTreeNode, ProgressEvent } from '../../types/index';

type Mode = 'upload' | 'download';

interface Props {
  appId: string;
  mode: Mode;
  description?: string;
  imagePaths?: string[];
  onClose: () => void;
  onDone: (result?: { backupTimestamp?: string }) => void;
  addToast: (m: { type: 'success' | 'error' | 'info' | 'warning'; message: string }) => void;
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function collectFilePaths(nodes: FileTreeNode[]): string[] {
  const paths: string[] = [];
  for (const n of nodes) {
    if (n.isDirectory) paths.push(...collectFilePaths(n.children));
    else paths.push(n.relativePath);
  }
  return paths;
}

function collectSelectedFilePaths(nodes: FileTreeNode[], selected: Set<string>): string[] {
  const paths: string[] = [];
  for (const n of nodes) {
    if (n.isDirectory) {
      paths.push(...collectSelectedFilePaths(n.children, selected));
    } else if (selected.has(n.relativePath)) {
      paths.push(n.relativePath);
    }
  }
  return paths;
}

function formatSize(bytes: number): string {
  if (bytes === 0) return '—';
  if (bytes < 1024) return `${bytes} o`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} Ko`;
  return `${(bytes / 1024 / 1024).toFixed(2)} Mo`;
}

function countFiles(nodes: FileTreeNode[]): number {
  return nodes.reduce((acc, n) => acc + (n.isDirectory ? countFiles(n.children) : 1), 0);
}

function totalSize(nodes: FileTreeNode[]): number {
  return nodes.reduce((acc, n) => acc + (n.isDirectory ? totalSize(n.children) : n.size), 0);
}

// ─── Tree node component ──────────────────────────────────────────────────────

function TreeNode({ node, selected, onToggle, depth = 0 }: {
  node: FileTreeNode;
  selected: Set<string>;
  onToggle: (node: FileTreeNode, checked: boolean) => void;
  depth?: number;
}) {
  const [expanded, setExpanded] = useState(depth < 2);

  const allChildFiles = node.isDirectory ? collectFilePaths([node]) : [];
  const checkedChildren = allChildFiles.filter(p => selected.has(p));
  const isChecked = node.isDirectory
    ? checkedChildren.length === allChildFiles.length
    : selected.has(node.relativePath);
  const isIndeterminate = node.isDirectory &&
    checkedChildren.length > 0 && checkedChildren.length < allChildFiles.length;

  const handleCheck = (e: React.ChangeEvent<HTMLInputElement>) => {
    onToggle(node, e.target.checked);
  };

  return (
    <div>
      <div style={{
        display: 'flex', alignItems: 'center', gap: 6,
        padding: '3px 8px 3px', paddingLeft: `${8 + depth * 18}px`,
        borderRadius: 4, cursor: 'pointer',
        background: 'transparent',
        transition: 'background 0.1s',
      }}
        onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--surfaceEmeraldBlueHover)')}
        onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
      >
        <input
          type="checkbox"
          checked={isChecked}
          ref={el => { if (el) el.indeterminate = isIndeterminate; }}
          onChange={handleCheck}
          style={{ width: 14, height: 14, accentColor: 'var(--green)', flexShrink: 0, cursor: 'pointer' }}
        />
        {node.isDirectory && (
          <span onClick={() => setExpanded(!expanded)}
            style={{ display: 'flex', alignItems: 'center', color: 'var(--accentEmerald)', cursor: 'pointer', flexShrink: 0 }}>
            <span style={{ fontSize: 10, marginRight: 2, display: 'inline-block', transform: expanded ? 'rotate(90deg)' : 'none', transition: 'transform 0.15s' }}>▶</span>
          </span>
        )}
        <span onClick={() => node.isDirectory && setExpanded(!expanded)}
          style={{ flex: 1, display: 'flex', alignItems: 'center', gap: 6, cursor: node.isDirectory ? 'pointer' : 'default' }}>
          <span style={{ fontSize: 14, flexShrink: 0 }}>{node.isDirectory ? (expanded ? '📂' : '📁') : '📄'}</span>
          <span style={{ fontSize: 12, color: 'var(--text)', wordBreak: 'break-all' }}>{node.name}</span>
          {!node.isDirectory && node.size > 0 && (
            <span style={{ fontSize: 11, color: 'var(--txtEmerald)', marginLeft: 'auto', flexShrink: 0 }}>
              {formatSize(node.size)}
            </span>
          )}
          {node.isDirectory && allChildFiles.length > 0 && (
            <span style={{ fontSize: 11, color: 'var(--txtEmerald)', marginLeft: 'auto', flexShrink: 0 }}>
              {checkedChildren.length}/{allChildFiles.length} fichier{allChildFiles.length > 1 ? 's' : ''}
            </span>
          )}
        </span>
      </div>
      {node.isDirectory && expanded && node.children.length > 0 && (
        <div>
          {node.children.map(child => (
            <TreeNode key={child.relativePath} node={child} selected={selected}
              onToggle={onToggle} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
}

// ─── Progress bar ──────────────────────────────────────────────────────────────

function ProgressBar({ event }: { event: ProgressEvent | null }) {
  const pct = event && event.total > 0 ? Math.round((event.current / event.total) * 100) : 0;
  const phases: Record<string, string> = {
    uploading: 'Upload…', downloading: 'Téléchargement…', finalizing: 'Finalisation…', scanning: 'Analyse…',
  };
  return (
    <div style={{ padding: '14px 20px', borderTop: '1px solid var(--border)' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 8, fontSize: 13 }}>
        <span style={{ color: 'var(--accentEmerald)' }}>{event ? (phases[event.phase] || event.phase) : 'En cours…'}</span>
        <span style={{ color: 'var(--accentEmerald)', fontWeight: 700 }}>{pct}%</span>
      </div>
      <div style={{ height: 6, background: 'var(--surfaceEmeraldGreen-dim)', borderRadius: 3, overflow: 'hidden' }}>
        <div style={{ height: '100%', width: `${pct}%`, background: 'var(--txtEmerald)', borderRadius: 3, transition: 'width 0.25s' }} />
      </div>
      {event?.currentFile && (
        <div style={{ marginTop: 5, fontSize: 11, color: 'var(--txtEmerald-dim)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {event.currentFile}
        </div>
      )}
    </div>
  );
}

// ─── Main modal ───────────────────────────────────────────────────────────────

export function SaveTreeModal({ appId, mode, description = '', imagePaths = [], onClose, onDone, addToast }: Props) {
  const ipc = useIpc();
  const [nodes, setNodes] = useState<FileTreeNode[]>([]);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [done, setDone] = useState(false);
  const [downloadNotes, setDownloadNotes] = useState<{ description: string; uploaderName: string; uploadedAt: string } | null>(null);

  const title = mode === 'upload' ? '☁️ Confirmer l\'upload' : '⬇️ Confirmer le téléchargement';
  const actionLabel = mode === 'upload' ? 'Uploader la sélection' : 'Télécharger la sélection';

  useEffect(() => {
    setLoading(true);
    const fetch = mode === 'upload'
      ? ipc.filetree.getLocalTree(appId)
      : ipc.filetree.getCloudTree(appId);
    fetch.then(r => {
      if (r.success && r.data) {
        const tree = r.data as FileTreeNode[];
        setNodes(tree);
        // All files selected by default
        setSelected(new Set(collectFilePaths(tree)));
      } else {
        addToast({ type: 'error', message: r.error || 'Impossible de charger l\'arborescence.' });
      }
    }).finally(() => setLoading(false));
  }, [appId, mode]);

  const handleToggle = useCallback((node: FileTreeNode, checked: boolean) => {
    setSelected(prev => {
      const next = new Set(prev);
      const paths = collectFilePaths([node]);
      if (!node.isDirectory) {
        if (checked) next.add(node.relativePath); else next.delete(node.relativePath);
      } else {
        paths.forEach(p => checked ? next.add(p) : next.delete(p));
      }
      return next;
    });
  }, []);

  const handleSelectAll = () => setSelected(new Set(collectFilePaths(nodes)));
  const handleSelectNone = () => setSelected(new Set());

  const selectedFilePaths = collectSelectedFilePaths(nodes, selected);
  const selectedCount = selectedFilePaths.length;
  const selectedBytes = nodes.reduce((acc, n) => acc + sumSelectedSize(n, selected), 0);

  const handleConfirm = async () => {
    if (selectedCount === 0) { addToast({ type: 'warning', message: 'Sélectionnez au moins un fichier.' }); return; }
    setRunning(true);
    try {
      if (mode === 'upload') {
        const r = await ipc.filetree.uploadSelected(
          { appId, selectedPaths: selectedFilePaths, description, imagePaths },
          (ev) => setProgress(ev)
        );
        if (r.success) {
          addToast({ type: 'success', message: `${selectedCount} fichier${selectedCount > 1 ? 's' : ''} uploadé${selectedCount > 1 ? 's' : ''} !` });
          onDone();
        } else {
          addToast({ type: 'error', message: r.error || 'Erreur upload' });
        }
      } else {
        const r = await ipc.filetree.downloadSelected(appId, selectedFilePaths, (ev) => setProgress(ev));
        if (r.success) {
          // Fetch notes to display in success screen
          const infoRes = await ipc.saves.getCloudInfo(appId);
          if (infoRes.success && infoRes.data) {
            const meta = infoRes.data as { notes: { description: string; uploaderName: string; uploadedAt: string } | null };
            if (meta.notes) setDownloadNotes(meta.notes);
          }
          setDone(true);
          onDone(r.data as any);
        } else {
          addToast({ type: 'error', message: r.error || 'Erreur téléchargement' });
        }
      }
    } finally { setRunning(false); }
  };

  const allCount = countFiles(nodes);

  return (
    <div className="modal-overlay" onClick={e => e.target === e.currentTarget && !running && onClose()}>
      <div className="modal modal--wide" style={{ maxHeight: '85vh', display: 'flex', flexDirection: 'column' }}>
        <div className="modal__header">
          <span className="modal__title">{title}</span>
          {!running && <button className="modal__close" onClick={onClose}>×</button>}
        </div>

        {/* Toolbar */}
        <div style={{ padding: '8px 20px', borderBottom: '1px solid var(--border)', display: 'flex', gap: 8, alignItems: 'center' }}>
          <button className="btn btn--sm btn--secondary" onClick={handleSelectAll} disabled={running}>Tout sélectionner</button>
          <button className="btn btn--sm btn--secondary" onClick={handleSelectNone} disabled={running}>Tout désélectionner</button>
          <span style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--text2)' }}>
            {selectedCount}/{allCount} fichier{allCount > 1 ? 's' : ''} · {formatSize(selectedBytes)}
          </span>
        </div>

        {/* Tree */}
        <div style={{ flex: 1, overflowY: 'auto', padding: '8px 12px' }}>
          {loading ? (
            <div style={{ textAlign: 'center', padding: 40, color: 'var(--text2)' }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 10 }}>
                <span className="btn__spinner" style={{ borderTopColor: 'var(--accent)', borderColor: 'var(--border)' }} />
                Chargement de l'arborescence…
              </div>
            </div>
          ) : nodes.length === 0 ? (
            <div style={{ textAlign: 'center', padding: 40, color: 'var(--text3)' }}>
              {mode === 'upload' ? 'Aucun fichier trouvé dans le dossier de sauvegarde.' : 'Aucun fichier sur le cloud pour ce jeu.'}
            </div>
          ) : (
            nodes.map(n => <TreeNode key={n.relativePath} node={n} selected={selected} onToggle={handleToggle} />)
          )}
        </div>

        {/* Progress, success screen, or footer */}
        {running && <ProgressBar event={progress} />}

        {!running && done && mode === 'download' && (
          <div style={{ padding: '20px', borderTop: '1px solid var(--border)' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 14 }}>
              <span style={{ fontSize: 20 }}>✅</span>
              <span style={{ fontWeight: 600, color: 'var(--green)', fontSize: 14 }}>
                Téléchargement terminé — backup local créé
              </span>
            </div>
            {downloadNotes ? (
              <div style={{
                background: 'var(--surface3)', borderRadius: 8,
                border: '1px solid var(--border)', padding: '14px 16px',
              }}>
                <div style={{ fontSize: 11, color: 'var(--text3)', marginBottom: 6, textTransform: 'uppercase', letterSpacing: '0.06em' }}>
                  Notes de la sauvegarde
                </div>
                <div style={{ fontSize: 13, color: 'var(--text2)', marginBottom: 8 }}>
                  Uploadée par <strong style={{ color: 'var(--text)' }}>{downloadNotes.uploaderName}</strong>
                  {' '}le {new Date(downloadNotes.uploadedAt).toLocaleString('fr-FR', {
                    day: '2-digit', month: '2-digit', year: 'numeric',
                    hour: '2-digit', minute: '2-digit'
                  })}
                </div>
                {downloadNotes.description ? (
                  <div style={{
                    fontSize: 13, color: 'var(--text)', lineHeight: 1.6,
                    padding: '10px 12px', background: 'var(--surface)',
                    borderRadius: 6, borderLeft: '3px solid var(--accent)',
                  }}>
                    {downloadNotes.description}
                  </div>
                ) : (
                  <div style={{ fontSize: 12, color: 'var(--text3)', fontStyle: 'italic' }}>
                    Aucune description pour cette sauvegarde.
                  </div>
                )}
              </div>
            ) : (
              <div style={{ fontSize: 13, color: 'var(--text3)', fontStyle: 'italic' }}>
                Aucune note disponible pour cette sauvegarde.
              </div>
            )}
            <div style={{ marginTop: 14, display: 'flex', justifyContent: 'flex-end' }}>
              <button className="btn btn--primary" onClick={onClose}>Fermer</button>
            </div>
          </div>
        )}

        {!running && !done && (
          <div className="modal__footer">
            <button className="btn btn--ghost" onClick={onClose}>Annuler</button>
            <button className="btn btn--primary" onClick={handleConfirm}
              disabled={selectedCount === 0 || loading}
              style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
              <Icon name={mode === 'upload' ? 'upload' : 'download'} size={14} />
              {actionLabel} ({selectedCount})
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function sumSelectedSize(node: FileTreeNode, selected: Set<string>): number {
  if (!node.isDirectory) return selected.has(node.relativePath) ? node.size : 0;
  return node.children.reduce((acc, c) => acc + sumSelectedSize(c, selected), 0);
}

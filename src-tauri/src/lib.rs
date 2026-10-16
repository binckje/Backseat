mod error;
mod state;
pub mod settings;
pub mod logger;
pub mod filetree;
mod services;
mod commands;

use state::AppState;
use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // App
            get_app_version, quit_app,
            // Settings
            get_settings, save_settings_cmd, detect_steam_path,
            // Steam
            get_owned_games, get_friends_list, get_player_info, detect_save_path,
            // Cloud
            cloud_connect, cloud_disconnect, cloud_get_status,
            gdrive_get_auth_url, gdrive_exchange_code,
            // Games
            get_shared_games, add_game, remove_game, update_save_path,
            // Permissions
            get_permissions, grant_access, revoke_access, release_lock,
            // Saves
            upload_save, download_save, get_cloud_info, download_images,
            // Backup
            list_backups, restore_backup, delete_backup_cmd,
            // Dialogs
            open_folder_dialog, open_images_dialog,
            // Cloud scan
            scan_cloud_games,
            // Heroic
            detect_heroic_path, get_heroic_games, suggest_heroic_save_paths,
            enrich_heroic_art,
            // File tree
            get_local_save_tree, get_cloud_save_tree,
            upload_selected_saves, download_selected_saves,
            // Debug/logs
            get_log_path, set_debug_logging, open_log_folder,
            // Bulk refresh
            refresh_all_games_status,
        ])
        .run(tauri::generate_context!())
        .expect("Error while running Backseat");
}

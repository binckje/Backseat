import { invoke, Channel } from '@tauri-apps/api/core';
import type { AppSettings, CloudProviderType, UploadPayload, ProgressEvent } from '../types/index';

async function call<T>(fn: Promise<T>): Promise<{ success: boolean; data?: T; error?: string }> {
  try { return { success: true, data: await fn }; }
  catch (err) { return { success: false, error: String(err) }; }
}

const api = {
  app: {
    quit:       () => call(invoke<void>('quit_app')),
    getVersion: () => call(invoke<string>('get_app_version')),
  },
  settings: {
    get:             () => call(invoke<AppSettings>('get_settings')),
    save:            (partial: Partial<AppSettings>) =>
                       call(invoke<AppSettings>('save_settings_cmd', { partial })),
    detectSteamPath: () => call(invoke<string>('detect_steam_path')),
  },
  steam: {
    getGames:       () => call(invoke('get_owned_games')),
    getFriends:     () => call(invoke('get_friends_list')),
    getPlayerInfo:  (steamId: string) => call(invoke('get_player_info', { steamId })),
    detectSavePath: (appId: string)   => call(invoke('detect_save_path', { appId })),
  },
  cloud: {
    connect:            (type: CloudProviderType) => call(invoke('cloud_connect', { providerType: type })),
    disconnect:         () => call(invoke('cloud_disconnect')),
    getStatus:          () => call(invoke('cloud_get_status')),
    test:               () => call(invoke('cloud_get_status')),
    gdriveGetAuthUrl:   () => call(invoke<string>('gdrive_get_auth_url')),
    gdriveExchangeCode: (code: string) => call(invoke('gdrive_exchange_code', { code })),
  },
  games: {
    getShared:      () => call(invoke('get_shared_games')),
    add:            (game: object) => call(invoke('add_game', { game })),
    remove:         (appId: string) => call(invoke('remove_game', { appId })),
    updateSavePath: (appId: string, newPath: string) => call(invoke('update_save_path', { appId, newPath })),
  },
  permissions: {
    get:         (appId: string) => call(invoke('get_permissions', { appId })),
    grant:       (appId: string, friendSteamId: string) => call(invoke('grant_access', { appId, friendSteamId })),
    revoke:      (appId: string, friendSteamId: string) => call(invoke('revoke_access', { appId, friendSteamId })),
    releaseLock: (appId: string) => call(invoke('release_lock', { appId })),
  },
  saves: {
    upload: (payload: UploadPayload, onProgress: (ev: ProgressEvent) => void) => {
      const channel = new Channel<ProgressEvent>();
      channel.onmessage = onProgress;
      return call(invoke('upload_save', { payload, onProgress: channel }));
    },
    download: (appId: string, onProgress: (ev: ProgressEvent) => void) => {
      const channel = new Channel<ProgressEvent>();
      channel.onmessage = onProgress;
      return call(invoke('download_save', { appId, onProgress: channel }));
    },
    getCloudInfo:   (appId: string) => call(invoke('get_cloud_info', { appId })),
    downloadImages: (appId: string) => call(invoke('download_images', { appId })),
  },
  status: {
    refreshAll: () => call(invoke('refresh_all_games_status')),
  },
  filetree: {
    getLocalTree:  (appId: string) => call(invoke('get_local_save_tree', { appId })),
    getCloudTree:  (appId: string) => call(invoke('get_cloud_save_tree', { appId })),
    uploadSelected: (payload: { appId: string; selectedPaths: string[]; description: string; imagePaths: string[] },
                     onProgress: (ev: ProgressEvent) => void) => {
      const channel = new Channel<ProgressEvent>();
      channel.onmessage = onProgress;
      return call(invoke('upload_selected_saves', { payload, onProgress: channel }));
    },
    downloadSelected: (appId: string, selectedPaths: string[], onProgress: (ev: ProgressEvent) => void) => {
      const channel = new Channel<ProgressEvent>();
      channel.onmessage = onProgress;
      return call(invoke('download_selected_saves', { appId, selectedPaths, onProgress: channel }));
    },
  },
  debug: {
    getLogPath:      () => call(invoke<string>('get_log_path')),
    setDebugLogging: (enabled: boolean) => call(invoke('set_debug_logging', { enabled })),
    openLogFolder:   () => call(invoke('open_log_folder')),
  },
  heroic: {
    detectPath:       () => call(invoke<string | null>('detect_heroic_path')),
    getGames:         () => call(invoke('get_heroic_games')),
    suggestSavePaths: (appName: string, platform: string) =>
                        call(invoke<string[]>('suggest_heroic_save_paths', { appName, platform })),
    enrichArt:        () => call(invoke<boolean>('enrich_heroic_art')),
  },
  cloud_scan: {
    scanGames: () => call(invoke('scan_cloud_games')),
  },
  backup: {
    list:    (appId: string) => call(invoke('list_backups', { appId })),
    restore: (appId: string, timestamp?: string) => call(invoke('restore_backup', { appId, timestamp })),
    delete:  (appId: string, timestamp: string) => call(invoke('delete_backup_cmd', { appId, timestamp })),
  },
  dialog: {
    openFolder: () => call(invoke<string | null>('open_folder_dialog')),
    openImages: () => call(invoke<string[]>('open_images_dialog')),
  },
};

export type BackseatAPI = typeof api;
export function useIpc(): BackseatAPI { return api; }

export interface SteamGame { appId: string; name: string; playtime: number; iconHash?: string; }
export interface SteamFriend { steamId: string; personaName: string; avatar: string; profileUrl: string; }
export interface SteamPlayer { steamId: string; personaName: string; avatar: string; profileUrl: string; }
export type SaveStatus = 'available' | 'locked' | 'not_uploaded';
export interface CloudSaveInfo {
  uploadedBy: string; uploaderName: string; uploadedAt: string; description: string;
  imageCount: number; status: SaveStatus; lockedBy: string | null; lockedAt: string | null;
}
export interface SharedGame {
  appId: string; name: string; savePath: string; savePathDetectionMethod: string;
  iconHash?: string; cloudInfo?: CloudSaveInfo; hasLocalBackup?: boolean; backupTimestamp?: string;
  launcher?: 'steam' | 'heroic';
  heroPlatform?: 'epic' | 'gog';
  coverUrl?: string;
  backgroundUrl?: string;
}

export interface HeroicGame {
  appName: string;
  title: string;
  platform: 'epic' | 'gog';
  installPath?: string;
  savePath?: string;
  winePrefix?: string;
  isNative: boolean;
  coverUrl?: string;
  backgroundUrl?: string;
}
export interface GamePermissions {
  appId: string; gameName: string; owner: string; allowedUsers: string[];
  status: SaveStatus; lockedBy: string | null; lockedAt: string | null;
  lastUpload: string | null; lastUploaderSteamId: string | null;
}
export type CloudProviderType = 'google_drive' | 'nextcloud' | 'kdrive';
export interface CloudConnectionStatus {
  connected: boolean; providerType: CloudProviderType | null;
  error?: string; userInfo?: { name: string; email?: string; quota?: string };
}
export interface AppSettings {
  steamApiKey: string; steamId: string; steamPath: string; backupPath: string;
  activeCloudProvider: CloudProviderType | null;
  cloudProviders: any;
  sharedGames: SharedGame[]; cachedPlayerInfo?: SteamPlayer;
}
export interface ProgressEvent {
  current: number; total: number; currentFile?: string;
  phase: 'scanning'|'backing_up'|'uploading'|'downloading'|'finalizing';
}
export interface IpcResult<T = void> { success: boolean; data?: T; error?: string; }
export type ModalType = 'settings'|'add_game'|'image_viewer'|null;
export interface ToastMessage { id: string; type: 'success'|'error'|'info'|'warning'; message: string; duration?: number; }

export interface CloudGameSuggestion {
  appId: string;
  gameName: string;
  isOwner: boolean;
}

export interface FileTreeNode {
  name: string;
  relativePath: string;
  isDirectory: boolean;
  size: number;
  /** Last modification time, Unix ms */
  modified?: number;
  children: FileTreeNode[];
}

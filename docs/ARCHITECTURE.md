# Backseat — carte du code

Référence pour reprendre le projet sans accès GitHub : chaque fichier, ses fonctions, qui les appelle et ce qu'elles appellent. Les numéros de ligne sont indicatifs (état de la v1.6.1) ; en cas de doute, chercher le nom de la fonction.

Pour demander une modif à Claude sans accès au repo : indiquer le fichier + la fonction (noms ci-dessous) et coller le contenu actuel de la fonction si possible. Claude pourra alors répondre avec un remplacement exact « remplacer ce bloc par celui-ci ».

---

## 1. Vue d'ensemble

```
React (src/)                         Rust (src-tauri/src/)
─────────────                        ─────────────────────
Composant ──▶ useIpc().xxx.yyy() ──invoke('commande')──▶ commands.rs::commande
                                                            │
                                     settings::load/save ◀──┤ (config.json + trousseau)
                                     state.provider() ◀─────┤ (provider cloud connecté)
                                     services::* ◀──────────┘
                                        ├─ steam / heroic   (détection jeux et chemins)
                                        ├─ permissions      (permissions_<slug>.json sur le cloud)
                                        ├─ saves            (upload / download / métadonnées)
                                        ├─ backup           (copie locale avant download)
                                        └─ cloud::CloudProvider (nextcloud.rs, googledrive.rs)
```

- Toutes les commandes IPC renvoient `Result<T, String>` côté Rust ; côté front, `call()` dans `useIpc.ts` les transforme en `{ success, data?, error? }`.
- Les noms passent automatiquement de `camelCase` (TS) à `snake_case` (Rust) pour les arguments (`appId` → `app_id`). Les structs Rust ont `#[serde(rename_all = "camelCase")]`.
- La progression des transferts passe par un `Channel` Tauri (`onProgress`) qui reçoit des `ProgressEvent { current, total, currentFile, phase }`.

### Arborescence cloud (`services/cloud/mod.rs`)
```
/backseat/<slug>_<appId>/              game_cloud_path  (slug = nom du jeu, caractères non alphanum → "_", 40 car. max)
    saves/...                          game_saves_path
    images/0.png, 1.jpg...             game_images_path
    notes.json                         game_notes_path  (NotesData)
    permissions_<slug>_<appId>.json    game_permissions_path (GamePermissions)
```

### Fichiers locaux
- Config : `~/.config/backseat/config.json` (`settings::config_path`). Secrets dans le trousseau OS (service `backseat`).
- Backups : `~/.backseat/backups/<steamId>/<appId>/<timestamp>/` ou `backupPath` des paramètres. 3 conservés.
- Logs : `~/.local/share/backseat/backseat.log` (+ `backseat.old.log`). Mode verbeux : `--verbose`/`--debug`/`-v`, `BACKSEAT_LOG=debug` ou `debugLogging: true`.
- Cache images : `<app_cache_dir>/image-cache/<appId>/`.

---

## 2. Rust — `src-tauri/src/`

### `main.rs`
| Fonction | Rôle |
|---|---|
| `main()` | Linux/Wayland : force `WEBKIT_DISABLE_DMABUF_RENDERER` et `WEBKIT_DISABLE_COMPOSITING_MODE`. Lit le mode verbeux (`logger::is_verbose_requested` ou `settings::load().debug_logging`), appelle `logger::init` puis `backseat_lib::run()`. |

### `lib.rs`
| Fonction | Rôle |
|---|---|
| `run()` | Construit l'app Tauri : plugins `opener` + `dialog`, état `AppState::new()`, et **la liste de toutes les commandes IPC** dans `generate_handler![...]`. **Toute nouvelle commande doit être ajoutée ici** (sinon erreur « command not found » côté front). |

### `state.rs`
| Élément | Rôle | Utilisé par |
|---|---|---|
| `CloudState { provider, provider_type }` | Provider connecté (`Arc<dyn CloudProvider>`) | `cloud_connect`, `cloud_disconnect`, `cloud_get_status` |
| `AppState { cloud: Mutex<CloudState> }` | État global géré par Tauri | toutes les commandes cloud via `State<'_, AppState>` |
| `AppState::provider_opt()` | Clone l'`Arc` et relâche le verrou ; `None` si pas connecté | `add_game`, `remove_game` |
| `AppState::provider()` | Idem mais erreur `"Not connected"` | toutes les autres commandes cloud |

### `error.rs`
`AppError` (`Io`, `Http`, `Cloud`, `Permission`, `NotFound`, `Config`, `Other`) + `type Result<T>`. Conversions auto depuis `io::Error`, `reqwest::Error`, `serde_json::Error`. Affichage préfixé (`"Not found: ..."`, `"Permission: ..."`), c'est ce texte qui arrive dans les toasts.

### `logger.rs`
| Fonction | Rôle | Appelée par |
|---|---|---|
| `log_path()` | Chemin du fichier log | `init`, `commands::get_log_path`, `commands::open_log_folder` |
| `init(verbose)` | Rotation `.old.log`, `WriteLogger` (fallback terminal), bannière de démarrage | `main` |
| `is_verbose_requested()` | Lit args CLI / `BACKSEAT_LOG` | `main` |

### `settings.rs`
| Élément | Rôle | Appelé par |
|---|---|---|
| `AppSettings` | `steamApiKey, steamId, steamPath, backupPath, activeCloudProvider, cloudProviders (JSON libre), sharedGames, debugLogging` | partout |
| `SharedGame` | `appId, name, savePath, savePathDetectionMethod, iconHash?, hasLocalBackup?, backupTimestamp?, launcher? ("steam"/"heroic"), heroPlatform?, coverUrl?, backgroundUrl?` | partout |
| `config_path()` | `~/.config/backseat/config.json` (crée le dossier) | `load`, `save` |
| `SECRETS` | Liste des champs secrets (pointeur JSON parent, champ) | `extract_secrets`, `inject_secrets` |
| `keychain_key(parent, field)` | Clé trousseau `"<parent>/<field>"` | idem |
| `extract_secrets(json)` | Retire les secrets du JSON et les met dans le trousseau (`secure::store`) ; les garde si trousseau indispo | `save` |
| `inject_secrets(json)` | Remet les secrets depuis le trousseau ; renvoie `true` si un secret en clair existait (migration) | `load` |
| `load()` | Lit config + secrets ; défaut si absent/invalide ; ré-enregistre si migration | quasi toutes les commandes, `main`, `patch` |
| `save(&settings)` | Écrit config sans secrets | commandes qui modifient l'état, `patch`, `load` (migration) |
| `patch(partial)` | Fusionne les clés de premier niveau d'un JSON partiel puis `save` | `commands::save_settings_cmd` |

Structure de `cloudProviders` attendue (voir `build_provider`) :
```json
{ "nextcloud":    { "nextcloud":   { "serverUrl": "", "username": "", "password": "" } },
  "kdrive":       { "kdrive":      { "username": "", "appPassword": "" } },
  "google_drive": { "googleDrive": { "clientId": "", "clientSecret": "", "accessToken": null, "refreshToken": null, "tokenExpiry": null } } }
```

### `filetree.rs`
| Fonction | Rôle | Appelée par |
|---|---|---|
| `FileTreeNode` | `name, relativePath (avec /), isDirectory, size, modified? (Unix ms), children` | `get_local_save_tree`, `get_cloud_save_tree` |
| `FileTreeNode::file_count / total_size` | Comptes récursifs (non utilisés par le front actuellement) | — |
| `parse_cloud_time(s)` | Date WebDAV RFC 2822 / RFC 3339 → ms | `build_cloud_subtree` |
| `build_local_tree(root)` | `[]` si dossier absent, sinon `read_dir_iterative` | `commands::get_local_save_tree` |
| `read_dir_iterative(root)` | Parcours itératif (pile explicite), anti-boucles symlink (chemins canonisés visités), `MAX_DEPTH = 128`, tri dossiers puis nom | `build_local_tree` |
| `write_at_path(top, path, nodes)` | Insère des enfants à un chemin d'indices | `read_dir_iterative` |
| `build_cloud_tree(cloud, saves_path)` | Arbre du dossier `saves/` cloud | `commands::get_cloud_save_tree` |
| `build_cloud_subtree(...)` | Récursif, sous-dossiers listés en parallèle (`join_all`), tri dossiers puis nom | `build_cloud_tree` |

### `commands.rs` — commandes IPC
Helpers : `type Res<T> = Result<T, String>`, `e(err)` = `err.to_string()`.

Motif récurrent : `settings::load()` → trouver le jeu dans `s.shared_games` par `app_id` (`"Game not found"`) → `state.provider()` → service.

| Commande | Appelée depuis (front) | Appelle |
|---|---|---|
| `get_app_version` | `App.tsx` (init), `Settings/VersionBadge` | `app.package_info()` |
| `quit_app` | `App.tsx` bouton Quitter | `app.exit(0)` |
| `get_settings` | `App.tsx` init, `Settings` (après connexion/sauvegarde) | `settings::load` |
| `save_settings_cmd(partial)` | `Settings` (`handleSave`, `handleConnect`) | `settings::patch` |
| `detect_steam_path` | `Settings` bouton détecter | `steam::detect_steam_path` |
| `get_owned_games` | `AddGameModal/SteamTab` | `steam::get_owned_games` |
| `get_friends_list` | `GameCard.loadFriends` | `steam::get_friends_list` |
| `get_player_info(steamId)` | (non utilisée par le front) | `steam::get_player_summaries` |
| `detect_save_path(appId)` | `AddGameModal/SteamTab.handleSelect`, `CloudSuggestModal.handleDetect` | `steam::detect_save_path` |
| `build_provider(type, s)` *(interne)* | `cloud_connect` | `NextcloudProvider::new` / `new_kdrive`, `GoogleDriveProvider::new` |
| `cloud_connect(providerType)` | `App.tsx` init (auto si `activeCloudProvider`), `Settings.handleConnect/handleGdriveCode` | `build_provider`, `provider.connect()`, met à jour `AppState`, sauve `active_cloud_provider` |
| `cloud_disconnect` | `Settings.handleDisconnect` | vide `AppState`, `active_cloud_provider = None` |
| `cloud_get_status` | `useIpc.cloud.getStatus/test` (non utilisées par les composants) | lit `AppState` |
| `gdrive_get_auth_url` | `Settings.handleConnect` (Google) | `GoogleDriveProvider::auth_url` |
| `gdrive_exchange_code(code)` | `Settings.handleGdriveCode` | `GoogleDriveProvider::exchange_code`, `settings::save` |
| `get_shared_games` | `App.tsx`, `AddGameModal`, `CloudSuggestModal`, `GameCard.handleRemoveGame` | `settings::load` |
| `add_game(game)` | `AddGameModal` (Steam + Heroic), `CloudSuggestModal.handleAdd` | refuse doublon ; si connecté `permissions::init` ; `settings::save` |
| `remove_game(appId)` | `GameCard.handleRemoveGame` | si connecté **supprime tout le dossier cloud du jeu** (`delete(game_cloud_path)`) ; retire de la config |
| `update_save_path(appId, newPath)` | `GameCard/SavePathSection.handleSave` | met `save_path_detection_method = "manual"`, `settings::save` |
| `get_permissions(appId)` | `GameCard.loadCloudData`, `GameCard.refreshPerms` | `permissions::read` (+ `init` si absent) |
| `grant_access(appId, friendSteamId)` | `GameCard.handleAddBySteamId/handleAddFriend` | `permissions::grant` |
| `revoke_access(appId, friendSteamId)` | `GameCard` (bouton × d'un utilisateur) | `permissions::revoke` |
| `release_lock(appId)` | `GameCard.handleReleaseLock` | `permissions::release_lock` |
| `uploader_name(s)` *(interne)* | `upload_selected_saves` | pseudo Steam via `steam::get_player_summaries`, sinon steamId |
| `get_cloud_info(appId)` | `GameCard.loadCloudData`, `SaveTreeModal` (après download) | `saves::fetch_metadata` |
| `download_images(appId)` | `GameCard.handleViewImages` | `saves::download_images` (cache local) |
| `list_backups(appId)` | `GameCard` (montage, après download) | `backup::list` |
| `restore_backup(appId, timestamp?)` | `GameCard.handleRestore` | `backup::restore` |
| `delete_backup_cmd(appId, timestamp)` | `GameCard.handleDeleteBackup` | `backup::delete_backup` |
| `save_image_as(path)` | `ImageViewer.download` (via `invoke` direct) | boîte « Enregistrer sous » + `tokio::fs::copy` ; `false` si annulé |
| `open_folder_dialog` | `AddGameModal`, `CloudSuggestModal.handleBrowse`, `Settings` (dossier backup), `SavePathSection.handleBrowse` | `dialog().pick_folder` |
| `open_images_dialog` | `GameCard` (bouton ajouter des images) | `dialog().pick_files` (jpg/jpeg/png/gif/webp) |
| `read_image(path)` | `LocalImage` (via `invoke` direct) | lit le fichier → data URL base64 ; extensions image seulement, 25 Mo max |
| `scan_cloud_games` | `App.tsx.scanCloud` (à chaque connexion) | `list_files("/backseat")` puis lit en parallèle chaque `permissions_*.json` ; garde ceux où l'utilisateur est owner/autorisé et pas encore ajoutés → `CloudGameSuggestion { appId, gameName, isOwner }` |
| `detect_heroic_path` | (non utilisée par les composants) | `heroic::detect_heroic_config_path` |
| `get_heroic_games` | `AddGameModal/HeroicTab` | `heroic::detect_heroic_config_path`, `get_all_games` |
| `suggest_heroic_save_paths(appName, platform)` | `AddGameModal/HeroicTab.handleSelect` | `heroic::get_all_games`, `suggest_save_paths` |
| `refresh_all_games_status` | `App.tsx.refreshAllStatuses` (à chaque connexion) | `permissions::read` en parallèle → `GameStatusUpdate { appId, status, lockedBy }` |
| `enrich_heroic_art` | `App.tsx` init | complète `coverUrl`/`backgroundUrl` des jeux Heroic depuis Heroic ; `true` si modifié |
| `get_log_path` | `Settings/DebugTab` | `logger::log_path` |
| `set_debug_logging(enabled)` | `Settings/DebugTab.handleToggleDebug` | sauve `debug_logging` (effet au redémarrage) |
| `open_log_folder` | `Settings/DebugTab` | `opener().open_path` |
| `get_local_save_tree(appId)` | `SaveTreeModal` (mode upload) | `filetree::build_local_tree(save_path)` |
| `get_cloud_save_tree(appId)` | `SaveTreeModal` (mode download) | `filetree::build_cloud_tree(game_saves_path)` |
| `upload_selected_saves(payload, onProgress)` | `SaveTreeModal.handleConfirm` (mode upload) | perms (read/init/can_write/locked_by_other), `saves::safe_join` sur chaque chemin, `saves::transfer_all` (upload_file), images une par une, `notes.json`, `permissions::mark_uploaded`. `uploaderName` = pseudo Steam via `uploader_name` |
| `download_selected_saves(appId, selectedPaths, onProgress)` | `SaveTreeModal.handleConfirm` (mode download) | perms (read/can_read/locked_by_other), `safe_join` (avant tout), `backup::create` (échec = abandon), `permissions::lock`, `saves::transfer_all` (download_file), `release_lock` si erreur. Renvoie `{ backupTimestamp }` |

Note : après un download, le verrou reste posé (statut `locked`) jusqu'au prochain upload (`mark_uploaded`) ou `release_lock`. C'est le principe « je prends la save, je la rends ».

### `services/mod.rs`
Déclare `steam`, `heroic`, `saves`, `backup`, `permissions`, `cloud`, `secure`.

### `services/secure.rs` (trousseau OS, crate `keyring`)
| Fonction | Rôle | Appelée par |
|---|---|---|
| `cache()` | Cache mémoire clé → `Option<valeur>` (évite D-Bus / prompts répétés) | `store`, `retrieve`, `remove` |
| `entry(key)` | `keyring::Entry("backseat", key)` | idem |
| `store(key, value)` | Écrit si différent du cache | `settings::extract_secrets` |
| `retrieve(key)` | Lit (cache d'abord) ; `None` si absent/indispo | `settings::inject_secrets` |
| `remove(key)` | Supprime | `settings::extract_secrets` |

### `services/steam.rs`
| Fonction | Rôle | Appelée par |
|---|---|---|
| `detect_steam_path()` | Windows : `C:\Program Files (x86)\Steam`… ; Linux : `~/.steam/steam`, `~/.local/share/Steam`, `~/.steam/root` | `commands::detect_steam_path` |
| `steam_id64_to_account_id(id)` | SteamID64 → ID 32 bits (dossier `userdata`) | `detect_save_path` |
| `detect_save_path(steam_path, steam_id, app_id)` | 1) `userdata/<id32>/<appId>/remote` non vide → `steam_userdata` ; 2) Linux : `steamapps/compatdata/<appId>/pfx/drive_c/users/steamuser` → `proton` ; 3) sinon renvoie le chemin userdata quand même | `commands::detect_save_path` |
| `get_owned_games(key, id)` | API `IPlayerService/GetOwnedGames` ; erreurs explicites si clé/ID manquants | `commands::get_owned_games` |
| `get_friends_list(key, id)` | API `GetFriendList` puis `get_player_summaries` (profil Steam public requis) | `commands::get_friends_list` |
| `get_player_summaries(key, ids)` | API `GetPlayerSummaries` par paquets de 100 | `get_friends_list`, `commands::get_player_info`, `commands::uploader_name` |

### `services/heroic.rs` (Epic via Legendary, GOG)
| Fonction | Rôle | Appelée par |
|---|---|---|
| `HeroicPlatform { Epic, Gog }`, `HeroicGame` | `appName, title, platform, installPath?, savePath?, winePrefix?, isNative, coverUrl?, backgroundUrl?` | — |
| `detect_heroic_config_path()` | `~/.config/heroic` ou Flatpak `~/.var/app/com.heroicgameslauncher.hgl/config/heroic` | `commands::detect_heroic_path/get_heroic_games/suggest_heroic_save_paths/enrich_heroic_art` |
| `load_gog_library(cfg)` | `store_cache/gog_library.json` → map appName → entrée (jaquettes, `gog_save_location`) | `read_gog_games` |
| `resolve_gog_save_path(template, prefix)` | Remplace `<?SAVED_GAMES?>`, `<?DOCUMENTS?>`, `<?APPDATA?>`, `<?LOCAL_APPDATA?>` par les chemins du préfixe Wine | `read_gog_games` |
| `read_epic_games(cfg)` | `legendaryConfig/legendary/installed.json` + jaquettes via `load_epic_library` | `get_all_games` |
| `load_epic_library(cfg)` | `store_cache/legendary_library.json` (clé `library`) | `read_epic_games` |
| `read_gog_games(cfg)` | `gog_store/installed.json` ; natif → `detect_gog_native_save_path`, Wine → `gog_save_location` résolu | `get_all_games` |
| `read_gog_wine_prefix(cfg, app)` | `GamesConfig/<app>.json` → `winePrefix` | `read_gog_games` |
| `normalize_path(p)` | Fusionne les `/` multiples et retire le `/` final | lecture des configs |
| `wine_user_base(prefix)` | `<prefix>/pfx/drive_c/users/steamuser` ou `<prefix>/drive_c/users/steamuser` | `resolve_gog_save_path`, `suggest_save_paths` |
| `detect_gog_native_save_path(folder, title)` | Essaie `~/.local/share/…`, `~/.config/…`, `~/Documents/…` | `read_gog_games` |
| `suggest_save_paths(game)` | Liste de chemins candidats selon plateforme / natif / Wine | `commands::suggest_heroic_save_paths` |
| `get_all_games(cfg)` | Epic + GOG | commandes Heroic |

### `services/saves.rs`
| Fonction | Rôle | Appelée par |
|---|---|---|
| `CONCURRENCY = 6` | Transferts simultanés | `transfer_all` |
| `transfer_all(jobs, op, on_done)` | Exécute `op(a, b)` sur chaque paire avec 6 en parallèle (`buffer_unordered`), callback après chaque fichier, s'arrête à la 1re erreur | `commands::upload_selected_saves/download_selected_saves` |
| `safe_join(root, rel)` | Joint un chemin relatif en refusant `..`, absolu, préfixe de lecteur, vide | `download_images`, `commands::upload_selected_saves/download_selected_saves` |
| `NotesData` | `description, uploadedBy, uploaderName, uploadedAt` (= `notes.json`) | `commands::upload_selected_saves`, `fetch_metadata` |
| `CloudSaveMetadata` | `{ notes?, imageCount }` | `fetch_metadata` |
| `fetch_metadata(cloud, name, id)` | Lit `notes.json` + compte les images en parallèle | `commands::get_cloud_info` |
| `download_images(cloud, name, id, cache)` | Télécharge toutes les images dans le cache, renvoie les chemins locaux | `commands::download_images` |

### `services/backup.rs`
| Fonction | Rôle | Appelée par |
|---|---|---|
| `backup_root(custom)` | `backupPath` ou `~/.backseat/backups` | `backup_dir` |
| `backup_dir(steam_id, app_id, custom)` | `<root>/<steamId>/<appId>` | toutes |
| `ts_now()` | Horodatage `YYYY-MM-DDTHH-MM-SS-mmmZ` | `create` |
| `create(...)` | Copie le dossier de save (si existe, sinon renvoie `""`) puis `prune(3)` | `commands::download_selected_saves` |
| `restore(..., ts?)` | Dernier backup si `ts` absent ; **vide le dossier de save** puis recopie | `commands::restore_backup` |
| `delete_backup(...)` | Supprime un backup | `commands::delete_backup_cmd` |
| `list(...)` | Dossiers commençant par `20`, triés du plus récent au plus ancien | `commands::list_backups`, `restore`, `prune` |
| `prune(..., keep)` | Supprime au-delà de `keep` | `create` |
| `copy_dir(src, dst)` | Copie récursive async | `create`, `restore` |

### `services/permissions.rs`
`GamePermissions { version, lastUpdated, appId, gameName, owner, allowedUsers[], status ("not_uploaded"|"available"|"locked"), lockedBy?, lockedAt?, lastUpload?, lastUploaderSteamId? }` — stocké en JSON sur le cloud.

| Fonction | Rôle | Appelée par |
|---|---|---|
| `read(cloud, name, id)` | Lit le JSON ; `None` si absent/illisible | `commands::*` (get_permissions, upload/download, refresh_all_games_status), et les fonctions ci-dessous |
| `write(cloud, name, id, p)` | Met `last_updated`, crée le dossier, écrit | toutes les fonctions de modification |
| `can_read` / `can_write` | owner ou dans `allowed_users` (identiques) | commandes upload/download |
| `locked_by_other(p, id)` | Verrou posé par quelqu'un d'autre | commandes upload/download, `lock` |
| `init(cloud, id, name, owner)` | Crée le fichier si absent (statut `not_uploaded`, owner = moi) | `add_game`, `get_permissions`, `upload_selected_saves` |
| `grant(..., steam_id, requester)` | Owner seulement ; ajoute à `allowed_users` | `commands::grant_access` |
| `revoke(...)` | Owner seulement ; retire + libère son verrou éventuel | `commands::revoke_access` |
| `lock(..., steam_id)` | Refuse si verrouillé par un autre ; statut `locked` | commandes download |
| `release_lock(..., steam_id)` | Autorisé au détenteur du verrou ou à l'owner ; statut `available` | `commands::release_lock`, rollback des downloads en erreur |
| `mark_uploaded(..., steam_id)` | Statut `available`, verrou retiré, `last_upload` | commandes upload |

### `services/cloud/mod.rs`
| Élément | Rôle |
|---|---|
| `CLOUD_ROOT = "/backseat"` + `game_*_path(...)` | Chemins cloud (voir §1) |
| `CloudFileInfo { name, path, size, is_directory, modified_at? }` | Résultat de `list_files` (`path` relatif à la racine WebDAV, ex. `/backseat/...`) |
| `UserInfo { name, email?, quota? }` | Résultat de `connect` |
| `trait CloudProvider` | `connect, disconnect, exists, list_files, upload_file, download_file, upload_text, download_text, delete, create_directory`. **Pour ajouter un provider : implémenter ce trait + l'ajouter dans `commands::build_provider` + l'UI dans `Settings`.** |

### `services/cloud/nextcloud.rs` (Nextcloud et kDrive, WebDAV via `reqwest`)
| Fonction | Rôle |
|---|---|
| `new(server_url, user, password)` | Client HTTP (timeout 60 s), cache `created_dirs` |
| `new_kdrive(user, app_password)` | Même chose avec `https://connect.drive.infomaniak.com` |
| `url(path)` | `<server>/remote.php/dav/files/<user>/<path>` |
| `dav_prefix()` | Préfixe des `href` PROPFIND |
| `parse_propfind(xml, req)` | Parse XML à la main (`<d:response>`, `href`, `collection`, `getcontentlength`, `getlastmodified`), ignore le dossier lui-même |
| `inner(text, open, close)` | Extrait le texte entre deux balises |
| `connect` | OCS `/ocs/v2.php/cloud/users/<user>` ; fallback `exists("/backseat")` |
| `exists` | PROPFIND Depth 0 |
| `list_files` | PROPFIND Depth 1 ; `[]` si erreur HTTP |
| `create_directory` | MKCOL segment par segment (201/405 OK), mémorisé dans `created_dirs` |
| `upload_file` | `create_directory(parent)` + PUT ; erreur si statut non 2xx |
| `download_file` | GET + crée les dossiers locaux |
| `upload_text` / `download_text` | PUT / GET de texte (JSON) |
| `delete` | DELETE (404 accepté) + purge du cache `created_dirs` |

### `services/cloud/googledrive.rs` (incomplet)
`GoogleDriveConfig { clientId, clientSecret, accessToken?, refreshToken?, tokenExpiry? }`. `auth_url`, `exchange_code`, `ensure_token` (refresh), `connect` (API `about`) fonctionnent. `find_id` existe mais non utilisé. `exists` renvoie toujours `true`, `list_files` `[]`, `create_directory` ne fait rien, et `upload_file/download_file/upload_text/download_text/delete` renvoient une erreur « not yet implemented ».

---

## 3. Front — `src/`

### `main.tsx`
Monte `<App />` dans `#root` (StrictMode) et importe `index.css`.

### `types/index.ts`
Interfaces miroir des structs Rust : `SteamGame`, `SteamFriend`/`SteamPlayer`, `SaveStatus`, `CloudSaveInfo`, `SharedGame`, `HeroicGame`, `GamePermissions`, `CloudProviderType` (`'google_drive'|'nextcloud'|'kdrive'`), `CloudConnectionStatus`, `AppSettings`, `ProgressEvent`, `IpcResult`, `ModalType` (`'settings'|'add_game'|'image_viewer'|null`), `ToastMessage`, `CloudGameSuggestion`, `FileTreeNode`. **Si on ajoute un champ à une struct Rust, l'ajouter ici aussi.**

### `hooks/useIpc.ts`
`call(promise)` → `{ success, data?, error? }`. `useIpc()` renvoie l'objet `api` :

| Groupe | Méthode → commande Rust |
|---|---|
| `app` | `quit` → `quit_app`, `getVersion` → `get_app_version` |
| `settings` | `get` → `get_settings`, `save(partial)` → `save_settings_cmd`, `detectSteamPath` → `detect_steam_path` |
| `steam` | `getGames` → `get_owned_games`, `getFriends` → `get_friends_list`, `getPlayerInfo` → `get_player_info`, `detectSavePath` → `detect_save_path` |
| `cloud` | `connect` → `cloud_connect`, `disconnect` → `cloud_disconnect`, `getStatus`/`test` → `cloud_get_status`, `gdriveGetAuthUrl` → `gdrive_get_auth_url`, `gdriveExchangeCode` → `gdrive_exchange_code` |
| `games` | `getShared` → `get_shared_games`, `add` → `add_game`, `remove` → `remove_game`, `updateSavePath` → `update_save_path` |
| `permissions` | `get` → `get_permissions`, `grant` → `grant_access`, `revoke` → `revoke_access`, `releaseLock` → `release_lock` |
| `saves` | `getCloudInfo` → `get_cloud_info`, `downloadImages` → `download_images`, `readImage` → `read_image` |
| `status` | `refreshAll` → `refresh_all_games_status` |
| `filetree` | `getLocalTree` → `get_local_save_tree`, `getCloudTree` → `get_cloud_save_tree`, `uploadSelected` → `upload_selected_saves`, `downloadSelected` → `download_selected_saves` (avec `Channel`) |
| `debug` | `getLogPath` → `get_log_path`, `setDebugLogging` → `set_debug_logging`, `openLogFolder` → `open_log_folder` |
| `heroic` | `detectPath` → `detect_heroic_path`, `getGames` → `get_heroic_games`, `suggestSavePaths` → `suggest_heroic_save_paths`, `enrichArt` → `enrich_heroic_art` |
| `cloud_scan` | `scanGames` → `scan_cloud_games` |
| `backup` | `list` → `list_backups`, `restore` → `restore_backup`, `delete` → `delete_backup_cmd` |
| `dialog` | `openFolder` → `open_folder_dialog`, `openImages` → `open_images_dialog` |

Exceptions : `LocalImage` et `ImageViewer` appellent `invoke('read_image')` / `invoke('save_image_as')` directement.

### `store/index.ts` (Zustand, `useStore`)
État : `settings`, `sharedGames`, `selectedAppId`, `cloudStatus`, `modal`, `modalData`, `toasts`, `isLoading`.

| Action | Rôle | Utilisée par |
|---|---|---|
| `setSettings` / `updateSettings` | Remplace / fusionne les paramètres | `App`, `Settings` |
| `setSharedGames(g)` | Remplace la liste **en conservant le `cloudInfo` déjà chargé** | `App`, `AddGameModal`, `CloudSuggestModal`, `GameCard` |
| `updateSharedGame(id, patch)` | Fusionne un jeu (badges de statut) | `App.refreshAllStatuses`, `GameCard` |
| `selectGame(id)` | Jeu affiché | `Sidebar`, `GameCard` (après suppression) |
| `setCloudStatus` | État de connexion | `App`, `Settings` |
| `openModal(type, data)` / `closeModal` | Modales globales | `App`, `Sidebar`, `GameCard` (image_viewer) |
| `addToast` (4 s par défaut) / `removeToast` | Notifications | partout / `Toast` |
| `setLoading` | Non utilisé actuellement | — |

### `App.tsx`
| Fonction | Rôle | IPC |
|---|---|---|
| `App()` | Barre de titre (déplacement via `getCurrentWindow`), boutons Paramètres/Quitter, indicateur `cloudBusy`, `<Sidebar/>`, `<GameCard/>`, modales selon `modal`, `<CloudSuggestModal>` si `showSuggest`, `<ToastContainer/>` | `app.quit` |
| `useEffect` init | Lit version + settings + jeux en parallèle, affiche l'UI ; en arrière-plan : `heroic.enrichArt` (puis relit les jeux) et `cloud.connect(activeCloudProvider)` | `app.getVersion`, `settings.get`, `games.getShared`, `heroic.enrichArt`, `cloud.connect` |
| `useEffect [cloudStatus.connected]` | À la connexion : `refreshAllStatuses` + `scanCloud` | — |
| `useEffect [settings]` | Ouvre les paramètres si pas de clé API Steam | — |
| `refreshAllStatuses()` | Met à jour `cloudInfo.status/lockedBy` de chaque jeu (badges sidebar) | `status.refreshAll` |
| `scanCloud()` | Ouvre `CloudSuggestModal` si des jeux partagés ne sont pas encore ajoutés | `cloud_scan.scanGames` |

### `components/Sidebar/index.tsx`
| Fonction | Rôle |
|---|---|
| `Sidebar()` | Liste triée/filtrée des jeux (`useMemo`), sélection (`selectGame`), bouton + → `openModal('add_game')` |
| `GameItem({ game, active, onClick })` | Ligne de jeu : jaquette (Heroic `coverUrl` ou icône Steam), badge de statut, fallback `onError` |

Pas d'IPC.

### `components/GameCard/index.tsx`
| Fonction | Rôle | IPC |
|---|---|---|
| `GameCard()` | Récupère le jeu sélectionné ; écran vide sinon | — |
| `GameCardInner({ game })` | Fiche complète du jeu | — |
| `loadCloudData()` | Permissions + métadonnées en parallèle, met à jour le badge | `permissions.get`, `saves.getCloudInfo` |
| `useEffect [appId, connected]` | Réinitialise et appelle `loadCloudData` | — |
| `handleRefresh()` | Bouton Actualiser | `loadCloudData` |
| `useEffect [appId]` | Liste des backups | `backup.list` |
| `refreshPerms()` | Relit les permissions | `permissions.get` |
| `handleReleaseLock()` | Libérer le verrou (détenteur ou owner) | `permissions.releaseLock` |
| `handleRestore(ts?)` / `handleDeleteBackup(ts)` | Backups locaux | `backup.restore` / `backup.delete` |
| `handleRemoveGame()` | Retire le jeu (après confirmation) et supprime son dossier cloud | `games.remove`, `games.getShared` |
| `loadFriends()` | Charge les amis Steam (une fois) | `steam.getFriends` |
| `handleAddBySteamId()` / `handleAddFriend(f)` | Donne l'accès | `permissions.grant` |
| (inline) bouton × utilisateur | Retire l'accès | `permissions.revoke` |
| (inline) bouton images | Choisir des captures pour l'upload | `dialog.openImages` |
| `handleViewImages()` | Télécharge les images (cache) puis `openModal('image_viewer', paths)` | `saves.downloadImages` |
| Bouton « Télécharger » | `setTreeModal('download')` | — |
| Bouton « Uploader » | Copie description + images puis `setTreeModal('upload')` | — |
| `<SaveTreeModal onDone>` | Après upload : reset + badge `available` ; après download : badge backup + relit backups ; puis `refreshPerms` | `backup.list` |
| `formatTs(ts)` | Horodatage de backup → date lisible | — |
| `SavePathSection(...)` | Affiche/modifie le dossier de sauvegarde (`handleBrowse`, `handleSave`, `handleCancel`) | `dialog.openFolder`, `games.updateSavePath` |

Variables utiles : `myId = settings.steamId`, `isOwner`, `isLockedByMe`, `isLockedByOther`, `saveStatus`.

### `components/SaveTreeModal/index.tsx`
Props : `appId, mode ('upload'|'download'), description, imagePaths, onClose, onDone(result?), addToast`.

| Fonction | Rôle |
|---|---|
| `collectFilePaths(nodes)` | Tous les chemins de fichiers d'un sous-arbre |
| `collectSelectedFilePaths(nodes, selected)` | Fichiers cochés |
| `formatSize(bytes)` / `formatDate(ms)` | Affichage |
| `countFiles` / `totalSize` / `sumSelectedSize` | Totaux pour l'en-tête |
| `TreeNode({ node, selected, onToggle, depth })` | Ligne avec case à cocher (état partiel pour les dossiers), dépliable |
| `ProgressBar({ event })` | Progression |
| `SaveTreeModal(...)` | Charge l'arbre (`filetree.getLocalTree` en upload, `filetree.getCloudTree` en download), tout coché par défaut ; `handleToggle`, `handleSelectAll`, `handleSelectNone` ; `handleConfirm` → `filetree.uploadSelected` ou `filetree.downloadSelected` (puis `saves.getCloudInfo` pour afficher les notes) → écran de succès → `onDone` |

### `components/AddGameModal/index.tsx`
| Fonction | Rôle | IPC |
|---|---|---|
| `AddGameModal({ onClose })` | Onglets Steam / Heroic | — |
| `SteamTab` | Liste des jeux possédés (`useEffect`), recherche ; `handleSelect` détecte le chemin ; `handleAdd` ajoute (`launcher: 'steam'`) puis relit la liste ; bouton Parcourir | `steam.getGames`, `steam.detectSavePath`, `games.add`, `games.getShared`, `dialog.openFolder` |
| `HeroicTab` | Jeux Heroic (filtre Epic/GOG + recherche) ; `handleSelect` récupère les chemins suggérés ; `handleAdd` ajoute (`launcher: 'heroic'`, `appId = appName`, `heroPlatform`, jaquettes) | `heroic.getGames`, `heroic.suggestSavePaths`, `games.add`, `games.getShared`, `dialog.openFolder` |
| `GameListPanel<T>(...)` | Liste générique sélectionnable (id, nom, sous-titre, icône, badge) | — |

### `components/CloudSuggestModal/index.tsx`
Props : `suggestions: CloudGameSuggestion[]`, `onClose`. Pour chaque jeu trouvé sur le cloud : choisir le dossier local (`handleDetect` → `steam.detectSavePath`, `handleBrowse` → `dialog.openFolder`) puis `handleAdd` → `games.add` + `games.getShared`.

### `components/Settings/index.tsx`
| Fonction | Rôle | IPC |
|---|---|---|
| `SettingsModal({ onClose })` | Onglets Steam, Cloud, Debug sur une copie locale `local` | — |
| `set(patch)` / `setProvider(type, patch)` | Modifie `local` / `local.cloudProviders[type][clé]` | — |
| `handleSave()` | Sauvegarde puis relit | `settings.save`, `settings.get` |
| `handleConnect(type)` | Google : sauve + ouvre l'URL d'auth ; sinon sauve + `cloud.connect` + relit settings + `setCloudStatus` | `settings.save`, `cloud.gdriveGetAuthUrl`, `cloud.connect`, `settings.get` |
| `handleGdriveCode()` | Échange le code Google puis connecte | `cloud.gdriveExchangeCode`, `cloud.connect`, `settings.get` |
| `handleDisconnect()` | Déconnecte | `cloud.disconnect`, `settings.get` |
| (inline) | Choix du dossier de backup, détection du chemin Steam | `dialog.openFolder`, `settings.detectSteamPath` |
| `VersionBadge()` | Affiche la version | `app.getVersion` |
| `DebugTab(...)` | Chemin du log, activer le mode verbeux, ouvrir le dossier | `debug.getLogPath`, `debug.setDebugLogging`, `debug.openLogFolder` |

### `components/common/`
| Fichier | Rôle |
|---|---|
| `Icon.tsx` | `Icon({ name, size, color })` : icônes SVG inline par nom (`game`, `settings`, `quit`, `lock`, `upload`, `download`, `folder`…). Ajouter une icône = ajouter une entrée dans la map du fichier. |
| `LocalImage.tsx` | `<img>` dont la source est obtenue via `invoke('read_image', { path })` (data URL). Ne pas revenir à `convertFileSrc`/`file://` (ne s'affichent pas sous Linux). |
| `ImageViewer.tsx` | `ImageViewerModal({ images, onClose })` : précédente/suivante, vignettes, `download()` → `invoke('save_image_as')` |
| `Toast.tsx` | `ToastContainer()` : affiche `toasts` du store, clic × → `removeToast` |

---

## 4. Flux principaux

**Démarrage** : `main.rs` → `lib::run` → `App.tsx` init (settings, jeux) → `cloud_connect` (si provider actif) → `refresh_all_games_status` + `scan_cloud_games`.

**Ajouter un jeu** : `Sidebar` + → `AddGameModal` → `detect_save_path` / `suggest_heroic_save_paths` → `add_game` (crée `permissions_*.json` si connecté) → `get_shared_games`.

**Upload** : `GameCard` bouton Uploader → `SaveTreeModal(mode upload)` → `get_local_save_tree` → sélection → `upload_selected_saves` → vérif droits/verrou → fichiers (6 en parallèle) → images → `notes.json` → `mark_uploaded` (statut `available`).

**Download** : `GameCard` bouton Télécharger → `SaveTreeModal(mode download)` → `get_cloud_save_tree` → sélection → `download_selected_saves` → vérif droits/verrou → `safe_join` → backup local (abandon si échec) → `lock` → fichiers → (rollback `release_lock` si erreur).

**Partage** : owner → `GameCard` Permissions → `grant_access` ; l'ami, à sa prochaine connexion, voit le jeu dans `CloudSuggestModal` (via `scan_cloud_games`).

---

## 5. Où modifier quoi

| Besoin | Fichiers |
|---|---|
| Nouvelle commande IPC | fonction `#[tauri::command]` dans `commands.rs` + ajout dans `generate_handler!` (`lib.rs`) + méthode dans `useIpc.ts` + types dans `types/index.ts` |
| Nouveau paramètre persistant | `AppSettings` (`settings.rs`, avec `#[serde(default)]`) + `AppSettings` (`types/index.ts`) + UI `Settings` ; si secret : ajouter dans `SECRETS` |
| Nouveau champ de jeu | `SharedGame` (Rust + TS) avec `Option` + `skip_serializing_if` pour rester compatible |
| Nouveau provider cloud | `services/cloud/<x>.rs` implémentant `CloudProvider` + `pub mod` dans `cloud/mod.rs` + `build_provider` + `CloudProviderType` + UI `Settings` |
| Règles de droits / verrou | `services/permissions.rs` |
| Chemins cloud | `services/cloud/mod.rs` (`game_*_path`) — attention : changer le slug rend les anciens dossiers introuvables |
| Détection des dossiers de save | `services/steam.rs::detect_save_path`, `services/heroic.rs::suggest_save_paths` |
| Nombre de transferts parallèles | `saves::CONCURRENCY` |
| Nombre de backups gardés | `backup::create` (`prune(..., 3, ...)`) |
| Permissions Tauri (plugins) | `src-tauri/capabilities/default.json` ; CSP dans `src-tauri/tauri.conf.json` |

## 6. Vérifier une modif en local
```bash
cd src-tauri && cargo check     # compile le Rust
cd .. && npx tsc --noEmit       # vérifie le TypeScript
npm run dev                     # lance l'appli en dev
```
Logs détaillés : lancer avec `BACKSEAT_LOG=debug npm run dev`, puis lire `~/.local/share/backseat/backseat.log` (ou Paramètres → Debug → Ouvrir le dossier).

## 7. Points connus
- Google Drive : provider incomplet (voir §2).
- Commandes exposées mais non appelées par les composants : `get_player_info`, `cloud_get_status`, `detect_heroic_path`.

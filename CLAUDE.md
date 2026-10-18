# Backseat — contexte projet

Fichier lu automatiquement par Claude Code au début de chaque conversation. Le tenir à jour après chaque tâche.

## Projet
Appli desktop **Tauri 2** (Rust + React/TS, Zustand, Vite), v1.6.1. Partage de sauvegardes de jeux Steam/Heroic (Epic, GOG) entre amis via un cloud perso (Nextcloud/WebDAV, kDrive, Google Drive). ~5 000 lignes.

## Structure
- `src-tauri/src/commands.rs` : toutes les commandes IPC (jeux, cloud, permissions, upload/download, backups, arbre de fichiers).
- `src-tauri/src/services/` : `steam`, `heroic`, `saves` (upload/download + `safe_join`), `backup` (copie locale avant download), `permissions` (droits + verrous en JSON sur le cloud), `cloud/` (trait `CloudProvider`, `nextcloud.rs`, `googledrive.rs`).
- `src-tauri/src/settings.rs` : config JSON dans `~/.config/backseat/config.json`.
- `src-tauri/src/filetree.rs` : parcours de fichiers avec protection contre les boucles de symlinks.
- `src/` : front React ; `hooks/useIpc.ts` fait le lien avec les commandes Rust ; composants en modales (`GameCard`, `SaveTreeModal`, `AddGameModal`…).

## Commandes
```bash
npm install && npm run dev        # dev
npm run build                     # AppImage/.deb/NSIS
./scripts/build-linux.sh             # build interactif Linux (pull -> AppImage/deb/rpm) ; dossier par défaut = parent du dossier scripts/, à confirmer
# Windows : powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1 (pull -> exe/msi)
cd src-tauri && cargo check       # vérif Rust (Linux : libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libdbus-1-dev)
npx tsc --noEmit                  # vérif TS
```

## Fait
- **Point 3** : les chemins relatifs venant du cloud passent par `saves::safe_join`, qui refuse `..`, les chemins absolus et les préfixes de lecteur. Appliqué dans `saves::download` et `download_selected_saves` (validation avant backup/verrou).
- **Point 8** : un échec de `backup::create` arrête maintenant le téléchargement (`download_save`, `download_selected_saves`) au lieu d'être ignoré.
- **Point 2** : secrets (clé API Steam, mots de passe Nextcloud/kDrive, `clientSecret`/tokens Google) stockés dans le trousseau OS via la crate `keyring` (`services/secure.rs`, cache mémoire). Branché de façon transparente dans `settings::load/save` (liste `SECRETS`) ; migration auto d'un ancien `config.json` en clair au premier chargement ; si le trousseau est indisponible, le secret reste dans le JSON (log warning). Linux : nécessite `libdbus-1-dev` au build et un Secret Service (GNOME Keyring/KWallet) à l'exécution.
- **Point 4** : CSP définie dans `tauri.conf.json` (`csp` + `devCsp` qui autorise en plus le serveur Vite `localhost:5173`). Scripts limités à `'self'` ; images `https:` autorisées (jaquettes Heroic sur des CDN variés, avatars Steam) ; `style-src 'unsafe-inline'` gardé pour les styles inline React/Vite.
- **Point 5** : doublon `scan_cloud_games` retiré de `generate_handler!` (`lib.rs`).
- **Point 6** : supprimé `src/App.tsx.bck` et les logos Epic/GOG inutilisés de `src-tauri/icons/` (restent : icônes de l'appli).
- **Images de sauvegarde** : lues côté Rust (`read_image`, extensions image uniquement, 25 Mo max) et affichées en data URL via `components/common/LocalImage.tsx`. Le protocole asset (`convertFileSrc`) a été retiré : il ne s'affichait pas dans la webview Linux. `file://` ne marche pas non plus. Visionneuse (`ImageViewer.tsx`) : fenêtre ajustée à l'image (taille réelle, défilement au-delà de l'écran) + bouton Télécharger (`save_image_as`, boîte d'enregistrement).
- **Dates dans l'explorateur** : `FileTreeNode.modified` (Unix ms ; mtime local, `getlastmodified` WebDAV côté cloud), affiché dans `SaveTreeModal`.
- **Bouton Actualiser** sur `GameCard` : relit permissions + notes/images du jeu (`loadCloudData`).
- **Perf** : UI affichée dès lecture config/jeux, connexion + statuts + scan cloud en arrière-plan (indicateur dans la barre de titre). `AppState::provider()` clone un `Arc` et relâche le verrou tout de suite (plus de sérialisation des opérations cloud). Transferts en flux parallèle (`saves::transfer_all`, `CONCURRENCY` = 6). Statuts, scan cloud, listings récursifs, métadonnées et images en parallèle (`join_all`). Nextcloud : cache des dossiers créés (avant : un MKCOL par segment de chemin pour chaque fichier) et erreur remontée si un PUT échoue.
- **Nettoyage** : `cargo check` et `tsc --noEmit` sans warning ni erreur. `tauri-plugin-shell` remplacé par `tauri-plugin-opener` (`open_log_folder`, capability `opener:default`). `UPLOAD_URL`/`find_id` gardés avec `#[allow(dead_code)]` pour le futur provider Google Drive. `upload_selected_saves` valide les chemins du front via `safe_join` (un chemin invalide fait échouer l'upload).
- **Doc** : `docs/ARCHITECTURE.md` = carte complète du code (fonctions, appelants, flux, où modifier quoi). La mettre à jour après chaque modif de structure.
- Branches : `claude/dreamy-pascal-e9xoo7` (points 3, 8, mergée PR #1), `claude/dazzling-knuth-0j3t6a` (points 2, 4, 5, 6 + fonctionnalités ci-dessus), `claude/wizardly-lamport-b1vot7` (nettoyage warnings/tsc, validation upload).

## À faire (par priorité)
1. **Point 7** : pas de tests, de CI ni de linter.
2. `@tauri-apps/plugin-shell` dans `package.json` n'est plus utilisé (à retirer avec `npm uninstall`).
3. Google Drive : `list_files`/`exists` sont des stubs (provider incomplet).

## Préférences utilisateur
Réponses en français, concises. Donner des étapes de test manuel quand Claude ne peut pas tester lui-même.

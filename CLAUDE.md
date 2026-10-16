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
- **Images de sauvegarde** : affichées via le protocole asset Tauri (`convertFileSrc`), feature `protocol-asset`, scope `$APPCACHE/image-cache/**` + fichiers choisis dans `open_images_dialog` (ajoutés dynamiquement au scope). `file://` ne marche pas dans la webview.
- **Dates dans l'explorateur** : `FileTreeNode.modified` (Unix ms ; mtime local, `getlastmodified` WebDAV côté cloud), affiché dans `SaveTreeModal`.
- **Bouton Actualiser** sur `GameCard` : relit permissions + notes/images du jeu (`loadCloudData`).
- **Perf** : UI affichée dès lecture config/jeux, connexion + statuts + scan cloud en arrière-plan (indicateur dans la barre de titre). `AppState::provider()` clone un `Arc` et relâche le verrou tout de suite (plus de sérialisation des opérations cloud). Transferts en flux parallèle (`saves::transfer_all`, `CONCURRENCY` = 6). Statuts, scan cloud, listings récursifs, métadonnées et images en parallèle (`join_all`). Nextcloud : cache des dossiers créés (avant : un MKCOL par segment de chemin pour chaque fichier) et erreur remontée si un PUT échoue.
- Branches : `claude/dreamy-pascal-e9xoo7` (points 3, 8, mergée PR #1), `claude/dazzling-knuth-0j3t6a` (points 2, 4, 5, 6 + fonctionnalités ci-dessus).

## À faire (par priorité)
1. **Point 7** : pas de tests, de CI ni de linter.
2. Warnings `cargo check` existants : `commands.rs:142,558,573`, `googledrive.rs:11,82`.
3. Optionnel : `upload_selected_saves` fait confiance aux chemins relatifs envoyés par le front (moins critique).
4. Erreurs `tsc` existantes dans `src/components/Settings/index.tsx` (`backupPath` en double, `r` implicite any).
5. Google Drive : `list_files`/`exists` sont des stubs (provider incomplet).

## Préférences utilisateur
Réponses en français, concises. Donner des étapes de test manuel quand Claude ne peut pas tester lui-même.

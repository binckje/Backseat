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
cd src-tauri && cargo check       # vérif Rust (Linux : libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev)
npx tsc --noEmit                  # vérif TS
```

## Fait
- **Point 3** : les chemins relatifs venant du cloud passent par `saves::safe_join`, qui refuse `..`, les chemins absolus et les préfixes de lecteur. Appliqué dans `saves::download` et `download_selected_saves` (validation avant backup/verrou).
- **Point 8** : un échec de `backup::create` arrête maintenant le téléchargement (`download_save`, `download_selected_saves`) au lieu d'être ignoré.
- Branche : `claude/dreamy-pascal-e9xoo7` (pas de PR).

## À faire (par priorité)
1. **Point 2** : secrets en clair dans `config.json` (clé API Steam, mot de passe Nextcloud/kDrive, `client_secret` Google). `services/secure.rs` existe mais n'est pas branché : ajouter la crate `keyring` dans `Cargo.toml`, déclarer `secure` dans `services/mod.rs`, migrer les secrets.
2. **Point 4** : `csp: null` dans `tauri.conf.json`, à définir une CSP.
3. **Point 5** : `scan_cloud_games` enregistré deux fois dans `generate_handler!` (`lib.rs`).
4. **Point 6** : supprimer `src/App.tsx.bck` et les icônes en double/mal nommées dans `src-tauri/icons/`.
5. **Point 7** : pas de tests, de CI ni de linter.
6. Warnings `cargo check` existants : `commands.rs:142,558,573`, `googledrive.rs:11,82`.
7. Optionnel : `upload_selected_saves` fait confiance aux chemins relatifs envoyés par le front (moins critique).

## Préférences utilisateur
Réponses en français, concises. Donner des étapes de test manuel quand Claude ne peut pas tester lui-même.

# Backseat (Tauri)

Application de partage de sauvegardes Steam via cloud personnel.
Réécriture Tauri — ~10 Mo au lieu de ~150 Mo, pas de Node.js requis à l'exécution.

## Prérequis

### Rust (toutes plateformes)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# Redémarrer le terminal, puis :
rustup default stable
```

### Linux (Arch/Manjaro)
```bash
sudo pacman -S webkit2gtk-4.1 libayatana-appindicator gtk3 openssl
```

### Linux (Ubuntu/Debian)
```bash
sudo apt install libwebkit2gtk-4.1-dev libssl-dev libgtk-3-dev \
                 libayatana-appindicator3-dev librsvg2-dev
```

### Windows
WebView2 est intégré nativement sur Windows 11 et Windows 10 récent.
Sinon il s'installe automatiquement au premier lancement.

---

## Développement

```bash
npm install
npm run dev        # Lance Vite + Tauri en mode développement
```

## Build final

### Linux → AppImage + .deb
```bash
npm run build
# Sortie : src-tauri/target/release/bundle/appimage/backseat_*.AppImage
#          src-tauri/target/release/bundle/deb/backseat_*.deb
```

### Windows → .exe installateur (NSIS)
```bash
npm run build
# Sortie : src-tauri/target/release/bundle/nsis/Backseat_*_x64-setup.exe
```

---

## Structure
```
backseat-tauri/
├── src/                    ← Frontend React (TypeScript)
│   ├── hooks/useIpc.ts     ← Pont Tauri invoke() → API
│   ├── components/         ← Composants UI
│   ├── store/              ← Zustand state
│   └── types/              ← Types partagés
└── src-tauri/              ← Backend Rust
    └── src/
        ├── commands.rs     ← Toutes les commandes Tauri (≈ ipc/index.ts)
        └── services/
            ├── cloud/nextcloud.rs  ← WebDAV (Nextcloud + kDrive) sans lib
            ├── cloud/googledrive.rs← Google Drive OAuth2
            ├── steam.rs            ← Steam Web API
            ├── saves.rs            ← Upload / Download
            ├── backup.rs           ← Backups locaux
            └── permissions.rs      ← Système de permissions
```

## Configuration
Au premier lancement, l'écran Paramètres s'ouvre automatiquement.
- **Steam** : renseignez votre clé API et votre Steam ID 64-bit
- **Cloud Drive** : connectez Nextcloud, kDrive ou Google Drive

#!/usr/bin/env bash
# Build interactif de Backseat sous Linux : clone/pull GitHub -> dépendances -> tauri build.
set -euo pipefail

REPO_URL="https://github.com/binckje/Backseat.git"

ask() { local q="$1" def="$2" r; read -rp "$q [$def] : " r; echo "${r:-$def}"; }
yes() { local r; read -rp "$1 [o/N] : " r; [[ "$r" =~ ^[oOyY]$ ]]; }

# 1. Dossier et branche
# Par défaut : le dossier parent de celui du script (racine du dépôt).
DEFAULT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
read -rp "Dossier du projet : $DEFAULT_DIR, confirmer ? [O/n] : " R
if [[ -z "$R" || "$R" =~ ^[oOyY]$ ]]; then
  DIR="$DEFAULT_DIR"
else
  read -rp "Dossier du projet : " DIR
  [[ -n "$DIR" ]] || { echo "Dossier vide" >&2; exit 1; }
fi
BRANCH=$(ask "Branche" "main")

if [[ -d "$DIR/.git" ]]; then
  echo "==> Mise à jour de $DIR"
  git -C "$DIR" fetch origin "$BRANCH"
  # Modifs locales (souvent écrites par un build précédent) : elles bloqueraient le pull.
  DIRTY=$(git -C "$DIR" status --porcelain --untracked-files=no)
  if [[ -n "$DIRTY" ]]; then
    echo "Modifications locales :"; echo "$DIRTY"
    yes "Les mettre de côté (git stash) pour continuer ?" || { echo "Mise à jour annulée (modifications locales)" >&2; exit 1; }
    git -C "$DIR" stash push -m "build-linux $(date -Iseconds)"
  fi
  git -C "$DIR" checkout "$BRANCH"
  git -C "$DIR" pull --ff-only origin "$BRANCH"
else
  echo "==> Clonage dans $DIR"
  git clone --branch "$BRANCH" "$REPO_URL" "$DIR"
fi
cd "$DIR"

# 2. Dépendances système (Tauri 2)
if yes "Installer/vérifier les dépendances système ?"; then
  if command -v apt-get >/dev/null; then
    sudo apt-get update
    sudo apt-get install -y build-essential curl wget file pkg-config \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
      librsvg2-dev libssl-dev libxdo-dev libdbus-1-dev
  elif command -v dnf >/dev/null; then
    sudo dnf install -y webkit2gtk4.1-devel openssl-devel dbus-devel curl wget file \
      libappindicator-gtk3-devel librsvg2-devel libxdo-devel
    sudo dnf group install -y "c-development"
  elif command -v pacman >/dev/null; then
    sudo pacman -Syu --needed webkit2gtk-4.1 base-devel curl wget file openssl \
      appmenu-gtk-module libappindicator-gtk3 librsvg xdotool
  else
    echo "Gestionnaire de paquets non reconnu : installer les dépendances Tauri à la main."
  fi
fi

# 3. Rust et Node
if ! command -v cargo >/dev/null; then
  echo "==> Installation de Rust (rustup)"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
if ! command -v npm >/dev/null; then
  echo "Node.js (>= 18) introuvable. Installer Node (ex. via nvm) puis relancer." >&2
  exit 1
fi
echo "Rust : $(rustc --version) | Node : $(node --version)"

# 4. Dépendances JS
npm ci || npm install

# 5. Mode
echo "1) Lancer en dev (npm run dev)"
echo "2) Build AppImage"
echo "3) Build .deb"
echo "4) Build AppImage + .deb + .rpm"
CHOICE=$(ask "Choix" "2")
case "$CHOICE" in
  1) exec npm run dev ;;
  2) BUNDLES="appimage" ;;
  3) BUNDLES="deb" ;;
  4) BUNDLES="appimage,deb,rpm" ;;
  *) echo "Choix invalide"; exit 1 ;;
esac

npm run build -- --bundles "$BUNDLES"

OUT="src-tauri/target/release/bundle"
echo "==> Terminé. Paquets :"
find "$OUT" -maxdepth 2 -type f \( -name '*.AppImage' -o -name '*.deb' -o -name '*.rpm' \) -print

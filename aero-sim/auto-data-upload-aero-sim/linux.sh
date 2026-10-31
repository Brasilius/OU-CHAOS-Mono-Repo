#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != Linux ]]; then
  echo 'This installer is for Linux. Use macos.sh or windows.ps1 on other systems.' >&2
  exit 1
fi
SOURCE_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${1:-}"
if [[ -z "$BINARY" ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo 'Install Rust from https://rustup.rs, or pass a prebuilt binary: ./linux.sh /path/to/aero-archive' >&2
    exit 1
  fi
  cargo build --release --locked --manifest-path "$SOURCE_DIR/Cargo.toml" --target-dir "$SOURCE_DIR/target"
  BINARY="$SOURCE_DIR/target/release/aero-archive"
fi
if [[ ! -f "$BINARY" ]]; then echo "Binary not found: $BINARY" >&2; exit 1; fi
"$BINARY" --version

DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
APP_DIR="$HOME/.local/lib/aero-archive"
mkdir -p "$APP_DIR" "$HOME/.local/bin" "$DATA_DIR/applications" "$DATA_DIR/icons/hicolor/scalable/apps"
# Replace by rename so an already-running executable is not truncated.
install -m 755 "$BINARY" "$APP_DIR/aero-archive.new"
mv -f "$APP_DIR/aero-archive.new" "$APP_DIR/aero-archive"
ln -sfn "$APP_DIR/aero-archive" "$HOME/.local/bin/aero-archive"
install -m 644 "$SOURCE_DIR/assets/aero-archive.svg" "$DATA_DIR/icons/hicolor/scalable/apps/aero-archive.svg"

# Escape for both the desktop-entry string grammar and the Exec argument grammar.
desktop_exec() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//\`/\\\`}"
  value="${value//\$/\\\$}"
  value="${value//%/%%}"
  value="${value//\\/\\\\}"
  printf '"%s"' "$value"
}
EXEC_PATH="$(desktop_exec "$APP_DIR/aero-archive")"
cat > "$DATA_DIR/applications/org.ouchaos.aeroarchive.desktop" <<DESKTOP
[Desktop Entry]
Version=1.0
Type=Application
Name=Aero Archive
Comment=Tag, version and upload simulation data to GitHub
Exec=$EXEC_PATH
Icon=aero-archive
Terminal=true
Categories=Science;Engineering;
Keywords=CFD;OpenRocket;simulation;GitHub;
DESKTOP
chmod 644 "$DATA_DIR/applications/org.ouchaos.aeroarchive.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$DATA_DIR/applications" || true; fi
echo "Installed: $APP_DIR/aero-archive"
echo 'Open Aero Archive from your application launcher, or run ~/.local/bin/aero-archive.'
echo 'A desktop terminal emulator and an XDG file-picker portal are needed for launcher and native dialog support.'
if ! command -v git >/dev/null 2>&1; then echo 'Git is missing. Install it with your package manager; Aero Archive will show setup steps on boot.'; fi

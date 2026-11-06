#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != Darwin ]]; then
  echo 'This installer is for macOS. Use linux.sh or windows.ps1 on other systems.' >&2
  exit 1
fi
SOURCE_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BINARY="${1:-}"
if [[ -z "$BINARY" ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo 'Install Rust from https://rustup.rs, or pass a prebuilt binary: ./macos.sh /path/to/aero-archive' >&2
    exit 1
  fi
  cargo build --release --locked --manifest-path "$SOURCE_DIR/Cargo.toml" --target-dir "$SOURCE_DIR/target"
  BINARY="$SOURCE_DIR/target/release/aero-archive"
fi
if [[ ! -f "$BINARY" ]]; then echo "Binary not found: $BINARY" >&2; exit 1; fi
"$BINARY" --version

APP="$HOME/Applications/Aero Archive.app"
if [[ -e "$APP" ]] && [[ ! -f "$APP/Contents/Resources/aero-archive-install" ]]; then
  echo "An unrelated application exists at $APP; move it before installing." >&2
  exit 1
fi
STAGING="$(mktemp -d "${TMPDIR:-/tmp}/aero-archive.XXXXXXXX")"
trap 'rm -rf -- "$STAGING"' EXIT
CONTENTS="$STAGING/Aero Archive.app/Contents"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources" "$HOME/Applications"
install -m 755 "$BINARY" "$CONTENTS/MacOS/aero-archive"
cat > "$CONTENTS/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Aero Archive</string>
<key>CFBundleDisplayName</key><string>Aero Archive</string>
<key>CFBundleIdentifier</key><string>org.ouchaos.aeroarchive</string>
<key>CFBundleExecutable</key><string>launch</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>LSMinimumSystemVersion</key><string>11.0</string>
</dict></plist>
PLIST
cat > "$CONTENTS/MacOS/launch" <<'LAUNCH'
#!/bin/sh
CONTENTS=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec /usr/bin/open -a Terminal "$CONTENTS/Resources/Aero Archive.command"
LAUNCH
cat > "$CONTENTS/Resources/Aero Archive.command" <<'TERMINAL'
#!/bin/sh
CONTENTS=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec "$CONTENTS/MacOS/aero-archive"
TERMINAL
chmod 755 "$CONTENTS/MacOS/launch" "$CONTENTS/Resources/Aero Archive.command"
touch "$CONTENTS/Resources/aero-archive-install"
# Copy only the app bundle. User settings and the managed data directory survive upgrades.
mkdir -p "$APP"
cp -R "$STAGING/Aero Archive.app/Contents" "$APP/"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP" || true
echo "Installed: $APP"
echo 'Open Aero Archive from Applications or Spotlight. It opens in Terminal.'
if ! git --version >/dev/null 2>&1; then echo 'Git is missing. Run xcode-select --install, then reopen the app for setup guidance.'; fi

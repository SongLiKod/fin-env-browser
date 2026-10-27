#!/usr/bin/env bash
set -euo pipefail

# Collect Tauri bundle artifacts, write SHA-256 checksums and a version file.
DEST="${1:-artifacts}"
mkdir -p "$DEST"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

find "$ROOT/src-tauri/target" -type f \
  \( -name '*.exe' -o -name '*.msi' -o -name '*.dmg' -o -name '*.AppImage' -o -name '*.deb' -o -name '*.tar.gz' \) \
  -size +1M -print0 2>/dev/null | while IFS= read -r -d '' file; do
    cp "$file" "$DEST/"
  done

VERSION="$(node -p "require('$ROOT/src-tauri/tauri.conf.json').version")"
echo "$VERSION" > "$DEST/VERSION"
( cd "$DEST" && sha256sum ./* > SHA256SUMS.txt )

echo "Artifacts:"
ls -lh "$DEST"

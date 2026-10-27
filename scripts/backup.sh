#!/usr/bin/env bash
set -euo pipefail

DATA_DIR="${FIN_ENV_DATA_DIR:-$HOME/.local/share/FinEnv}"
DEST="${1:-finenv-local-$(date +%Y%m%d-%H%M%S).tar.gz}"

if [ ! -d "$DATA_DIR" ]; then
  echo "FinEnv data directory not found: $DATA_DIR" >&2
  exit 1
fi

echo "Backing up $DATA_DIR -> $DEST"
tar -czf "$DEST" -C "$DATA_DIR" .
echo "Done: $DEST"

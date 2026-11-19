#!/usr/bin/env bash
# Genereert de app-iconen uit assets/logo.svg: assets/showmate.ico (Windows, meerdere formaten)
# en assets/icon-256.png (vensterpictogram). Vereist node + playwright en ImageMagick (`convert`).
set -euo pipefail
cd "$(dirname "$0")/.."
tmp=$(mktemp -d)
node tools/make-icons.js "$tmp"
convert "$tmp/icon-16.png" "$tmp/icon-20.png" "$tmp/icon-24.png" "$tmp/icon-32.png" "$tmp/icon-40.png" \
        "$tmp/icon-48.png" "$tmp/icon-64.png" "$tmp/icon-128.png" "$tmp/icon-256.png" assets/showmate.ico
cp "$tmp/icon-256.png" assets/icon-256.png
cp "$tmp/icon-512.png" assets/icon-512.png
rm -rf "$tmp"
echo "assets/showmate.ico, icon-256.png en icon-512.png bijgewerkt"

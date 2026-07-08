#!/usr/bin/env bash
# install-fonts.sh — install the exact monospace face the tapes record with.
#
# The VHS tapes set FontFamily "JetBrainsMono Nerd Font Mono". If that face is
# not installed, VHS silently falls back to whatever monospace fontconfig
# resolves, which renders the picker's box-drawing/geometric glyphs (◍ ◆ ● ○
# ⇥ ⏎ ─ │) at the wrong cell width — producing spread-out letters and drifting
# columns in the recording. Run this once per machine before `make regen`.
#
# Idempotent: exits early if the family is already present. User-level install
# (~/.local/share/fonts), no root required.
set -euo pipefail

FAMILY="JetBrainsMono Nerd Font Mono"
VERSION="v3.2.1"
URL="https://github.com/ryanoasis/nerd-fonts/releases/download/${VERSION}/JetBrainsMono.zip"
DEST="${HOME}/.local/share/fonts/JetBrainsMono"

# `grep -c` (not `-q`): under `set -o pipefail`, grep -q's early exit makes
# fc-list die with SIGPIPE and fails the whole pipeline even on a match.
if [ "$(fc-list : family 2>/dev/null | grep -ci 'JetBrainsMono Nerd Font Mono')" -gt 0 ]; then
  echo "✓ '${FAMILY}' already installed"
  exit 0
fi

echo "Installing '${FAMILY}' (nerd-fonts ${VERSION})…"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

curl -fsSL -o "$tmp/JetBrainsMono.zip" "$URL"
unzip -o -q "$tmp/JetBrainsMono.zip" -d "$tmp/extract"

mkdir -p "$DEST"
# Only the fixed-width "Mono" variants — Propo/NL have variable advance widths.
cp "$tmp"/extract/*NerdFontMono*.ttf "$DEST"/

fc-cache -f "$DEST" >/dev/null 2>&1 || fc-cache -f >/dev/null 2>&1

if [ "$(fc-list : family 2>/dev/null | grep -ci 'JetBrainsMono Nerd Font Mono')" -gt 0 ]; then
  echo "✓ Installed to ${DEST}"
else
  echo "✗ Install completed but fontconfig still can't see '${FAMILY}'." >&2
  echo "  Check that ${DEST} is on your fontconfig path." >&2
  exit 1
fi

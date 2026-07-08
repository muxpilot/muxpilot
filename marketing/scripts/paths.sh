#!/usr/bin/env bash
# paths.sh — single source of truth for marketing pipeline locations.
#
# The bash counterpart of a `paths.ts` module in a typical TS video pipeline,
# but terminal-native: constants + an `ensure` helper that creates a directory
# and echoes it.
#
# Source this from any pipeline script:  . "$(dirname "$0")/paths.sh"
set -euo pipefail

# Resolve repo root from this file's location (marketing/scripts/paths.sh).
_PATHS_SELF="${BASH_SOURCE[0]}"
SCRIPTS_DIR="$(cd "$(dirname "$_PATHS_SELF")" && pwd)"
MARKETING_DIR="$(cd "$SCRIPTS_DIR/.." && pwd)"
ROOT_DIR="$(cd "$MARKETING_DIR/.." && pwd)"

# Inputs
TAPES_DIR="$MARKETING_DIR/tapes"
THEMES_DIR="$MARKETING_DIR/themes"

# Working / raw capture area (git-ignored scratch).
RAW_DIR="$MARKETING_DIR/.raw"

# Final destination: the Next.js site's public media folder, so the site can
# embed the outputs directly at /media/<name>.
MEDIA_DIR="$ROOT_DIR/apps/web/public/media"

# The release binary the tapes drive (when built).
CARGO_BIN="$ROOT_DIR/target/release/muxpilot"

# ensure <dir> — mkdir -p and echo the path.
ensure() {
  mkdir -p "$1"
  printf '%s\n' "$1"
}

# Exported for child scripts that don't source this file.
export SCRIPTS_DIR MARKETING_DIR ROOT_DIR TAPES_DIR THEMES_DIR RAW_DIR MEDIA_DIR CARGO_BIN

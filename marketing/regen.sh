#!/usr/bin/env bash
# regen.sh — regenerate all MuxPilot demo media. The single entry point.
#
# Idempotent: safe to run any time the TUI changes. It (1) resolves a muxpilot
# binary (real release build if present, else a mock stand-in), (2) runs every
# .tape through vhs, and (3) drops gif/mp4/png into apps/web/public/media so the
# site embeds them at /media/<name>.
#
#   ./regen.sh                 # render every tape
#   ./regen.sh picker.tape     # render one tape (by name or path)
#   MUXPILOT_BUILD=0 ./regen.sh  # never invoke cargo; use existing binary or mock
#
# Requires: vhs, ttyd, ffmpeg. See marketing/README.md.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/paths.sh
. "$HERE/scripts/paths.sh"
# shellcheck source=scripts/compose.sh
. "$HERE/scripts/compose.sh"

echo "MuxPilot demo regen"
echo "  media -> $MEDIA_DIR"
echo
echo "Checking dependencies:"
configure_sandbox
if ! check_deps; then
  echo
  echo "Aborting: install the missing tools above, then re-run." >&2
  exit 1
fi

echo
printf 'Resolving muxpilot binary: '
resolve_binary

# Map bare tape names to full paths under tapes/.
tapes=()
for arg in "$@"; do
  case "$arg" in
    /*) tapes+=("$arg") ;;
    *.tape) tapes+=("$TAPES_DIR/$arg") ;;
    *) tapes+=("$TAPES_DIR/$arg.tape") ;;
  esac
done

echo
echo "Rendering tapes:"
if render_tapes "${tapes[@]}"; then
  echo
  echo "✓ Done. Outputs in $MEDIA_DIR:"
  ls -1 "$MEDIA_DIR" 2>/dev/null | sed 's/^/    /' || true
else
  echo
  echo "✗ One or more tapes failed to render." >&2
  exit 1
fi

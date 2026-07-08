#!/usr/bin/env bash
# narrate.sh — optional voiceover — terminal-native voiceover hook.
#
# Muxpilot demos are silent by default. This is an OPTIONAL hook: if a TTS engine
# is available it can synthesize a short narration track for a given clip. It is
# gated on tooling and never required by regen.sh's default path.
#
# Usage: narrate.sh <text> <out.wav>
set -euo pipefail

TEXT="${1:-}"
OUT="${2:-}"
if [ -z "$TEXT" ] || [ -z "$OUT" ]; then
  echo "usage: narrate.sh <text> <out.wav>" >&2
  exit 64
fi

if command -v say >/dev/null 2>&1; then            # macOS
  say -o "$OUT" --data-format=LEF32@22050 "$TEXT"
elif command -v espeak-ng >/dev/null 2>&1; then    # Linux
  espeak-ng -w "$OUT" "$TEXT"
elif command -v piper >/dev/null 2>&1; then        # neural TTS, if installed
  printf '%s' "$TEXT" | piper --output_file "$OUT"
else
  echo "narrate: no TTS engine (say/espeak-ng/piper) — skipping narration" >&2
  exit 0
fi
echo "narrate: wrote $OUT"

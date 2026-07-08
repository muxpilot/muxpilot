#!/usr/bin/env bash
# compose.sh — the tape orchestrator — terminal-native tape orchestrator.
#
# Resolves a `muxpilot` binary onto PATH (real release binary if present, else
# the mock stand-in), then runs every requested .tape through vhs so outputs land
# in the site's media dir. Sourced by regen.sh; not run directly.
set -euo pipefail

# Assumes paths.sh already sourced (ROOT_DIR, MARKETING_DIR, TAPES_DIR, RAW_DIR,
# MEDIA_DIR, CARGO_BIN, ensure).

# Ensure mise-installed tools (vhs, ttyd) are reachable even under a bare shell.
if [ -d "$HOME/.local/share/mise/shims" ]; then
  case ":$PATH:" in
    *":$HOME/.local/share/mise/shims:"*) : ;;
    *) PATH="$HOME/.local/share/mise/shims:$PATH" ;;
  esac
fi
export PATH

# configure_sandbox — VHS drives a headless Chromium to render frames. On hosts
# where unprivileged user namespaces are restricted (Ubuntu's
# kernel.apparmor_restrict_unprivileged_userns=1, common on 6.8 kernels) the
# Chromium sandbox cannot start and VHS crashes. VHS honors VHS_NO_SANDBOX for
# exactly this case. We only render our own trusted TUI, so it is safe to enable
# automatically when the restriction is detected. Override by exporting
# VHS_NO_SANDBOX yourself.
configure_sandbox() {
  if [ -n "${VHS_NO_SANDBOX:-}" ]; then return; fi
  local restricted=0
  if [ "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns 2>/dev/null || echo 0)" = "1" ]; then
    restricted=1
  elif ! unshare --user --map-root-user true 2>/dev/null; then
    restricted=1
  fi
  if [ "$restricted" = "1" ]; then
    export VHS_NO_SANDBOX=1
    echo "  note: unprivileged userns restricted — setting VHS_NO_SANDBOX=1"
  fi
}

# check_deps — verify vhs and its runtime deps; return non-zero if missing.
check_deps() {
  local ok=0
  for tool in vhs ttyd ffmpeg; do
    if command -v "$tool" >/dev/null 2>&1; then
      printf '  ✓ %s (%s)\n' "$tool" "$(command -v "$tool")"
    else
      printf '  ✗ %s — missing\n' "$tool"
      ok=1
    fi
  done
  # The recording font must be present or VHS silently falls back and the
  # picker's glyphs render at the wrong width. Warn (don't fail) — install-fonts.sh fixes it.
  # NB: no `grep -q` here — it exits on first match, fc-list then dies with
  # SIGPIPE, and under `set -o pipefail` the pipeline would falsely report the
  # font missing. Reading the whole stream with `grep -c` avoids that.
  if [ "$(fc-list : family 2>/dev/null | grep -ci 'JetBrainsMono Nerd Font Mono')" -gt 0 ]; then
    printf '  ✓ font (JetBrainsMono Nerd Font Mono)\n'
  else
    printf '  ✗ font — "JetBrainsMono Nerd Font Mono" missing; run: bash scripts/install-fonts.sh\n'
  fi
  if [ "$ok" -ne 0 ]; then
    cat <<'EOF'

  VHS and its runtime deps are required to render tapes:
    mise use -g vhs@latest ttyd@latest    # ttyd + vhs
    # ffmpeg: apt install ffmpeg  /  brew install ffmpeg
  (or: go install github.com/charmbracelet/vhs@latest)
    # recording font:
    bash scripts/install-fonts.sh         # JetBrainsMono Nerd Font Mono
EOF
  fi
  return $ok
}

# resolve_binary — put a `muxpilot` on PATH inside RAW_DIR/bin.
# Prefers, in order: a `muxpilot` already on PATH, the release binary at
# CARGO_BIN, a fresh `cargo build --release`, and finally the mock stand-in.
resolve_binary() {
  local bindir; bindir="$(ensure "$RAW_DIR/bin")"
  local shim="$bindir/muxpilot"
  rm -f "$shim"

  if command -v muxpilot >/dev/null 2>&1 && [ "$(command -v muxpilot)" != "$shim" ]; then
    ln -sf "$(command -v muxpilot)" "$shim"
    echo "real (PATH: $(command -v muxpilot))"
  elif [ -x "$CARGO_BIN" ]; then
    ln -sf "$CARGO_BIN" "$shim"
    echo "real ($CARGO_BIN)"
  elif command -v cargo >/dev/null 2>&1 && [ "${MUXPILOT_BUILD:-1}" = "1" ]; then
    # Guarded release build. Set MUXPILOT_BUILD=0 to skip.
    if (cd "$ROOT_DIR" && cargo build --release -p muxpilot >/dev/null 2>&1) && [ -x "$CARGO_BIN" ]; then
      ln -sf "$CARGO_BIN" "$shim"
      echo "real (built $CARGO_BIN)"
    else
      _install_mock_shim "$shim"; echo "mock (cargo build failed)"
    fi
  else
    _install_mock_shim "$shim"; echo "mock (no binary; using stand-in)"
  fi

  case ":$PATH:" in *":$bindir:"*) : ;; *) PATH="$bindir:$PATH" ;; esac
  export PATH
}

_install_mock_shim() {
  cat > "$1" <<EOF
#!/usr/bin/env bash
exec "$SCRIPTS_DIR/mock-picker.sh" "\$@"
EOF
  chmod +x "$1"
}

# render_tapes [tape ...] — run the given tapes (default: all) through vhs.
render_tapes() {
  ensure "$MEDIA_DIR" >/dev/null
  local tapes=("$@")
  if [ ${#tapes[@]} -eq 0 ]; then
    # Default set = every deterministic demo tape. real-agents.tape needs a live
    # OpenRouter session, so it is driven only by scripts/real-agent.sh.
    tapes=()
    for t in "$TAPES_DIR"/*.tape; do
      [ "$(basename "$t")" = "real-agents.tape" ] && continue
      tapes+=("$t")
    done
  fi
  local rc=0
  for tape in "${tapes[@]}"; do
    printf '\n▶ vhs %s\n' "$(basename "$tape")"
    # Run from MARKETING_DIR so `Source themes/...` and `Output ../apps/...` resolve.
    if (cd "$MARKETING_DIR" && vhs "$tape"); then
      :
    else
      printf '  ✗ failed: %s\n' "$tape"; rc=1
    fi
  done
  return $rc
}

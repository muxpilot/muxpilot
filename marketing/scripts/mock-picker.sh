#!/usr/bin/env bash
# mock-picker.sh — a stand-in for `muxpilot demo --count <N>`.
#
# The real deterministic demo command (`muxpilot demo --count N`) is implemented
# in the Rust crate. Until a release binary is on PATH, regen.sh routes the tapes
# through THIS script so `make regen` still produces real, animated media that
# looks like the picker. It emulates the interactions the tapes exercise:
#
#   j/k, g/G   move selection
#   /          enter filter mode; type to filter; Esc leaves
#   Tab        cycle search scope
#   ?          toggle help overlay
#   q / Esc    quit
#
# It is intentionally a faithful *visual* stand-in, not the real engine — the
# data is fixed and there is no tmux integration. Colors track the Tasker
# design tokens (terracotta accent, warm neutrals, semantic green/amber).
set -uo pipefail

# ── args ────────────────────────────────────────────────────────────────────
COUNT=8
while [ $# -gt 0 ]; do
  case "$1" in
    demo) shift ;;
    --count) COUNT="${2:-8}"; shift 2 ;;
    --count=*) COUNT="${1#--count=}"; shift ;;
    *) shift ;;
  esac
done

# ── colors (256-color, matched to Tasker tokens) ────────────────────────────
ESC=$'\e'
RESET="${ESC}[0m"
ACCENT="${ESC}[38;5;173m"      # terracotta ~ #C2613F
ACCENT_BG="${ESC}[48;5;52m"    # deep warm bg for selection
SUCCESS="${ESC}[38;5;71m"      # green ~ #4E9A51
WARNING="${ESC}[38;5;172m"     # amber ~ #BE8420
TEXT="${ESC}[38;5;223m"        # warm off-white
DIM="${ESC}[38;5;244m"         # muted
FAINT="${ESC}[38;5;240m"       # tertiary / borders
BOLD="${ESC}[1m"

# ── data: group|glyph|name|windows|chip|chipkind|when ───────────────────────
ROWS=(
  "Running|cur|web-dashboard|1w|active|active|now"
  "Running|run|api-gateway|3w|3 agent|agent|now"
  "Running|run|devservers|4w|active|active|3h"
  "Running|run|payments-service|7w|2 agent|agent|19h"
  "Running|run|data-pipeline|2w|1 agent|agent|2d"
  "Configured|cfg|design-system||layout|cfg|—"
  "Configured|cfg|notification-worker||layout|cfg|—"
  "Configured|cfg|infra-terraform-modules||layout|cfg|—"
)
SCOPES=(all sessions agents projects dirs)

# ── state ───────────────────────────────────────────────────────────────────
sel=0
scope=0
filter=""
filtering=0
help=0

cleanup() { printf '%s[?25h%s[?1049l' "$ESC" "$ESC"; stty echo 2>/dev/null || true; }
trap cleanup EXIT INT TERM

# rows visible under the current filter + scope, as indices into ROWS.
visible_indices() {
  local i field name chip out=()
  for i in "${!ROWS[@]}"; do
    IFS='|' read -r _ _ name _ _ chipkind _ <<<"${ROWS[$i]}"
    # scope filter
    case "${SCOPES[$scope]}" in
      agents)   [ "$chipkind" = agent ] || continue ;;
      projects) [ "$chipkind" = cfg ]   || continue ;;
      sessions) [ "$chipkind" = cfg ]   && continue ;;
      dirs)     [ "$chipkind" = cfg ]   || continue ;;
      all)      : ;;
    esac
    # text filter (case-insensitive substring)
    if [ -n "$filter" ]; then
      shopt -s nocasematch
      [[ "$name" == *"$filter"* ]] || { shopt -u nocasematch; continue; }
      shopt -u nocasematch
    fi
    out+=("$i")
  done
  printf '%s\n' "${out[@]}"
}

glyph() {
  case "$1" in
    cur) printf '%s◆%s' "$ACCENT" "$RESET" ;;
    run) printf '%s●%s' "$SUCCESS" "$RESET" ;;
    cfg) printf '%s○%s' "$FAINT" "$RESET" ;;
  esac
}

chip() {
  local text="$1" kind="$2"
  case "$kind" in
    agent)  printf '%s◍ %s%s' "$WARNING" "$text" "$RESET" ;;
    active) printf '%s%s%s' "$SUCCESS" "$text" "$RESET" ;;
    cfg)    printf '%s%s%s' "$DIM" "$text" "$RESET" ;;
  esac
}

draw() {
  local -a vis
  mapfile -t vis < <(visible_indices)
  local total=${#vis[@]}
  [ "$sel" -ge "$total" ] && sel=$((total > 0 ? total - 1 : 0))

  printf '%s[H%s[2J' "$ESC" "$ESC"   # home + clear

  # top bar
  printf '  %s%smuxpilot%s   %sdemo · %d sessions%s' \
    "$BOLD" "$TEXT" "$RESET" "$DIM" "$COUNT" "$RESET"
  printf '   %sscope: %s%s%s\n' "$DIM" "$ACCENT" "${SCOPES[$scope]}" "$RESET"
  printf '  %s%s%s\n\n' "$FAINT" "────────────────────────────────────────────────────────────" "$RESET"

  if [ "$help" -eq 1 ]; then
    printf '  %s%sHELP%s\n\n' "$BOLD" "$ACCENT" "$RESET"
    printf '    %sEnter%s  open selected workspace\n' "$TEXT" "$RESET"
    printf '    %sj / k%s  move selection\n' "$TEXT" "$RESET"
    printf '    %sg / G%s  jump to first / last\n' "$TEXT" "$RESET"
    printf '    %s/%s      filter    %sTab%s  cycle scope\n' "$TEXT" "$RESET" "$TEXT" "$RESET"
    printf '    %s?%s      toggle help    %sq%s  close\n\n' "$TEXT" "$RESET" "$TEXT" "$RESET"
    printf '  %spress ? to return%s\n' "$DIM" "$RESET"
    return
  fi

  # rows, grouped
  local last_group="" idx name windows chiptext chipkind when g gl
  local vi=0
  for idx in "${vis[@]}"; do
    IFS='|' read -r g gl name windows chiptext chipkind when <<<"${ROWS[$idx]}"
    if [ "$g" != "$last_group" ]; then
      [ -n "$last_group" ] && printf '\n'
      printf '  %s%s%s\n' "$FAINT" "$(printf '%s' "$g" | tr '[:lower:]' '[:upper:]')" "$RESET"
      last_group="$g"
    fi
    # Pad plain text to fixed widths FIRST, then colorize — otherwise printf
    # counts the ANSI escapes toward the field width and the columns jitter.
    local namecol wincol chipcol whencol rowstyle=""
    printf -v namecol '%-22s' "$name"
    printf -v wincol '%3s' "$windows"
    printf -v chipcol '%-9s' "$chiptext"
    printf -v whencol '%4s' "$when"
    [ "$vi" -eq "$sel" ] && rowstyle="$ACCENT_BG"
    printf '  %s%s %s%s%s %s%s%s  %s%s%s  %s%s%s%s\n' \
      "$rowstyle" "$(glyph "$gl")" \
      "$TEXT" "$namecol" "$RESET$rowstyle" \
      "$DIM" "$wincol" "$RESET$rowstyle" \
      "$rowstyle" "$(chip "$chipcol" "$chipkind")" "$RESET$rowstyle" \
      "$DIM" "$whencol" "$RESET" "$RESET"
    vi=$((vi + 1))
  done
  [ "$total" -eq 0 ] && printf '  %sno matches%s\n' "$DIM" "$RESET"

  # filter line + footer
  printf '\n'
  if [ "$filtering" -eq 1 ]; then
    printf '  %s/%s%s%s█\n' "$ACCENT" "$RESET" "$TEXT" "$filter"
  else
    printf '  %s⏎ open   / filter   d dirs   ⇥ scope   ? help   q close%s\n' "$DIM" "$RESET"
  fi
}

# ── input loop ──────────────────────────────────────────────────────────────
printf '%s[?1049h%s[?25l' "$ESC" "$ESC"   # alt screen, hide cursor
stty -echo 2>/dev/null || true

mapfile -t _vis < <(visible_indices)
count_vis=${#_vis[@]}

draw
while true; do
  IFS= read -rsn1 key || break
  if [ "$filtering" -eq 1 ]; then
    case "$key" in
      "$ESC") filtering=0 ;;                       # Esc leaves filter
      $'\x7f') filter="${filter%?}" ;;             # Backspace
      "") : ;;                                     # Enter: keep filter, stay
      *) filter="${filter}${key}" ;;
    esac
    sel=0
    draw
    continue
  fi
  case "$key" in
    q) break ;;
    j) sel=$((sel + 1)) ;;
    k) sel=$((sel > 0 ? sel - 1 : 0)) ;;
    g) sel=0 ;;
    G) mapfile -t _v < <(visible_indices); sel=$(( ${#_v[@]} - 1 )) ;;
    /) filtering=1 ;;
    '?') help=$((1 - help)) ;;
    $'\t') scope=$(((scope + 1) % ${#SCOPES[@]})); sel=0 ;;
    "$ESC")
      # swallow escape sequences (arrows) or quit on bare Esc
      read -rsn2 -t 0.05 rest || rest=""
      if [ -z "$rest" ]; then break; fi
      case "$rest" in
        "[A") sel=$((sel > 0 ? sel - 1 : 0)) ;;
        "[B") sel=$((sel + 1)) ;;
      esac ;;
    "") : ;;  # Enter = "open"; the demo just holds
  esac
  # clamp
  mapfile -t _v < <(visible_indices)
  [ "$sel" -ge "${#_v[@]}" ] && sel=$(( ${#_v[@]} - 1 ))
  [ "$sel" -lt 0 ] && sel=0
  draw
done

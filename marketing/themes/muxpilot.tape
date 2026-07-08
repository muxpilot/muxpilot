# muxpilot.tape — shared VHS settings sourced by every tape.
# Fixes terminal size, font, padding, and a warm Tasker-ish dark theme so every
# recording is deterministic and on-brand. Sourced with `Source themes/muxpilot.tape`
# (paths are relative to the marketing/ dir, where regen.sh runs vhs).
#
# Font: JetBrainsMono Nerd Font Mono — install once with marketing/scripts/install-fonts.sh.
# The Nerd Font "Mono" variant renders the picker's box-drawing and geometric
# glyphs (◍ ◆ ● ○ ⇥ ⏎ ─ │) at a true single cell width, so columns never drift
# and there is no fallback letter-spacing.

Set Shell "bash"
Set FontFamily "JetBrainsMono Nerd Font Mono"
Set FontSize 26
Set Width 1180
Set Height 640
Set Padding 48
Set Margin 0
Set TypingSpeed 55ms
Set PlaybackSpeed 1.0

# Warm near-black ground with a terracotta accent — mirrors the site's dark theme
# (page #17120E, text #E8E1D5, accent #F0705E).
Set Theme { "name": "MuxPilot", "background": "#17120E", "foreground": "#E8E1D5", "cursor": "#F0705E", "selection": "#3A241E", "black": "#241C16", "red": "#EF5F50", "green": "#5BBE69", "yellow": "#F0A93C", "blue": "#7AA7D8", "magenta": "#C88BB0", "cyan": "#69B7B0", "white": "#B7AD9E", "brightBlack": "#524638", "brightRed": "#F0705E", "brightGreen": "#5BBE69", "brightYellow": "#F0A93C", "brightBlue": "#8FB8E0", "brightMagenta": "#D89BBE", "brightCyan": "#7FC7C0", "brightWhite": "#E8E1D5" }

# navigate.tape — navigation + the tab bar. Moves through rows with j/k and g/G,
# uses the digit-prefix jump (a number then ↑/↓ hops that many rows — the
# left gutter shows each row's distance), then switches tabs by letter
# (a / l / d / s) — the top tab bar highlights the active mode.
Source themes/muxpilot.tape

Output ../apps/web/public/media/navigate.gif
Output ../apps/web/public/media/navigate.mp4

# Launch off-camera so the clip opens on the picker, not a shell typing.
Hide
Type "muxpilot demo --count 14"
Enter
Sleep 1.5s
Show

# Walk down, jump to the bottom, jump back to the top.
Type "jjj"
Sleep 700ms
Type "G"
Sleep 800ms
Type "g"
Sleep 800ms

# Digit-prefix jump: type a number, watch the gutter, then hop that many rows.
Type "5"
Sleep 600ms
Down
Sleep 900ms
Type "3"
Sleep 600ms
Up
Sleep 900ms

# Switch tabs by letter: Agents -> Layouts -> Dirs -> back to Sessions.
Type "a"
Sleep 1s
Type "l"
Sleep 1s
Type "d"
Sleep 1s
Type "s"
Sleep 900ms

Type "q"
Sleep 400ms

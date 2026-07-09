# navigate.tape — navigation + the tab bar. Moves through rows with j/k and g/G,
# then switches tabs by letter (a / x / d / s) — the top tab bar highlights the
# active mode, and each tab is its own focused list.
Source themes/muxpilot.tape

Output ../apps/web/public/media/navigate.gif
Output ../apps/web/public/media/navigate.mp4

Type "muxpilot demo --count 14"
Enter
Sleep 1s

# Walk down, jump to the bottom, jump back to the top.
Type "jjj"
Sleep 700ms
Type "G"
Sleep 800ms
Type "g"
Sleep 800ms

# Switch tabs by letter: Agents -> Layouts -> Dirs -> back to Sessions.
Type "a"
Sleep 1s
Type "x"
Sleep 1s
Type "d"
Sleep 1s
Type "s"
Sleep 900ms

Type "q"
Sleep 400ms

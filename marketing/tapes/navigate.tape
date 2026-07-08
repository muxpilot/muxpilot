# navigate.tape — navigation + scopes. Moves through rows with j/k and g/G, then
# cycles the search scope with Tab to filter by kind of target.
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

# Cycle the search scope: all -> sessions -> agents -> projects.
Tab
Sleep 800ms
Tab
Sleep 800ms
Tab
Sleep 1s

# Back to all, then close.
Tab
Tab
Sleep 600ms
Type "q"
Sleep 400ms

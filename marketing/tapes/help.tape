# help.tape — the help overlay. Opens the picker and toggles the ? help panel.
Source themes/muxpilot.tape

Output ../apps/web/public/media/help.gif
Output ../apps/web/public/media/help.mp4

Type "muxpilot demo --count 14"
Enter
Sleep 1s

# Toggle help open, hold so the keys are readable, toggle closed.
Type "?"
Sleep 2s
Type "?"
Sleep 700ms

Type "q"
Sleep 400ms

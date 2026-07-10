# filter.tape — fuzzy filtering. Opens the picker, enters filter mode, types a
# query to narrow the list, then clears it. Used in the docs.
Source themes/muxpilot.tape

Output ../apps/web/public/media/filter.gif
Output ../apps/web/public/media/filter.mp4

# Launch off-camera so the clip opens on the picker, not a shell typing.
Hide
Type "muxpilot demo --count 14"
Enter
Sleep 1.5s
Show

# Enter filter mode and type a query — watch the list narrow.
Type "/"
Sleep 500ms
Type "agent"
Sleep 1.2s

# Backspace out the query to widen again.
Backspace 5
Sleep 400ms
Type "run"
Sleep 1s

# Leave filter mode, then close.
Escape
Sleep 500ms
Escape
Type "q"
Sleep 400ms

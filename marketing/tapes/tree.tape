# tree.tape — session→windows tree expansion, on its own. Moves to a running
# session, expands it with `l`, walks into the window children, then collapses
# with `h`. Embedded in the docs as the feature clip.
Source themes/muxpilot.tape

Output ../apps/web/public/media/tree.gif
Output ../apps/web/public/media/tree.mp4

Type "muxpilot demo --count 14"
Enter
Sleep 1.2s

# Drop onto a running session that has windows, then expand it.
Type "jj"
Sleep 600ms
Type "l"
Sleep 1s

# Step into the window children.
Type "j"
Sleep 500ms
Type "j"
Sleep 700ms
Screenshot ../apps/web/public/media/tree.png
Type "j"
Sleep 700ms

# Collapse the session again (cursor jumps back to it).
Type "h"
Sleep 900ms

Type "q"
Sleep 400ms

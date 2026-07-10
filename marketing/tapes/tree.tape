# tree.tape — session→windows→panes tree, showing the adaptive expansion. Jumps
# to the top session, expands it, and lets the agent panes auto-reveal: each
# window that hosts a coding agent shows that pane's agent + model + status
# inline, without a second keystroke. Embedded in the docs as the feature clip.
Source themes/muxpilot.tape

Output ../apps/web/public/media/tree.gif
Output ../apps/web/public/media/tree.mp4

# Launch off-camera so the clip opens on the picker, not a shell typing.
Hide
Type "muxpilot demo --count 14"
Enter
Sleep 1.5s
Show

# Jump to the top session and expand it. Windows appear, and every window with a
# coding agent auto-reveals that pane (agent + model + status) beneath it.
Type "g"
Sleep 500ms
Type "l"
Sleep 1.2s
Screenshot ../apps/web/public/media/tree.png

# Walk down through the windows and their revealed agent panes.
Type "j"
Sleep 500ms
Type "j"
Sleep 500ms
Type "j"
Sleep 700ms

# Fully expand a window to show its plain shell panes too, then collapse back.
Type "l"
Sleep 900ms
Type "h"
Sleep 700ms

# Collapse the session again (cursor jumps back to it).
Type "g"
Sleep 300ms
Type "h"
Sleep 900ms

Type "q"
Sleep 400ms

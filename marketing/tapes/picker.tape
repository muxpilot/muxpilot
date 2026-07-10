# picker.tape — the hero demo embedded on the landing page.
# Opens the picker, captures a poster of the grouped list, walks the selection,
# then expands a running session into its windows (the tree feature) and
# collapses it again. Outputs gif + mp4 + png into the site's media dir. The
# count is tuned to fill the frame from `themes/muxpilot.tape`.
Source themes/muxpilot.tape

Output ../apps/web/public/media/picker.gif
Output ../apps/web/public/media/picker.mp4

# Launch the demo picker off-camera (Hide/Show), so the recording opens ON the
# picker instead of a shell slowly typing the command. Enough rows to fill frame.
Hide
Type "muxpilot demo --count 14"
Enter
Sleep 1.5s
Show

# Poster: the clean grouped list.
Screenshot ../apps/web/public/media/picker.png
Sleep 700ms

# Walk the selection down the running group.
Type "j"
Sleep 450ms
Type "j"
Sleep 650ms

# Expand this session into its windows, step through a couple, then collapse.
Type "l"
Sleep 1s
Type "j"
Sleep 500ms
Type "j"
Sleep 600ms
Type "h"
Sleep 800ms

# Close.
Type "q"
Sleep 400ms

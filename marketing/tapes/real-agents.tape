# real-agents.tape — records the REAL muxpilot picker showing live coding agents.
# Driven by scripts/real-agent.sh AFTER it has launched a detached tmux session
# with real OpenRouter agents running. Unlike the demo tapes, this runs the real
# binary against real tmux state, so it needs a built muxpilot + a live session.
Source themes/muxpilot.tape

Output ../apps/web/public/media/real-agents.gif
Output ../apps/web/public/media/real-agents.mp4

# Launch the real picker (reads the live tmux server).
Type "muxpilot"
Enter
Sleep 1.5s

# Narrow to just the agents scope so live agent state is front and centre.
Tab
Tab
Sleep 1.5s

# Walk the running agents.
Type "j"
Sleep 800ms
Type "j"
Sleep 1.5s

Type "q"
Sleep 400ms

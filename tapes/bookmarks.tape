# bookmarks.tape — Ctrl+B to pin a session
# Run: vhs tapes/bookmarks.tape

Output bookmarks.gif
Output bookmarks.webm

Set Shell bash
Set FontSize 16
Set Width 1400
Set Height 800
Set Theme "Catppuccin Mocha"
Set Padding 30
Set TypingSpeed 50ms
Set CursorBlink false

Sleep 500ms
Type "bash demo-mode.sh"
Sleep 600ms
Enter
Sleep 2000ms

# Enter architex
Enter
Sleep 2000ms

# Navigate down to auth-refactor
Down
Sleep 300ms
Down
Sleep 300ms
Down
Sleep 1000ms

# Ctrl+B to bookmark (in real tool this re-sorts; in demo-mode it's visual)
Ctrl+B
Sleep 2000ms

# Navigate to show the pinned item is at the top
Up
Sleep 500ms
Up
Sleep 500ms
Up
Sleep 500ms
Up
Sleep 1500ms

# Back out
Escape
Sleep 1500ms

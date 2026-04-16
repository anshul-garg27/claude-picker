# export.tape — Ctrl+E to export a session to markdown
# Run: vhs tapes/export.tape

Output export.gif
Output export.webm

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

# Navigate to auth-refactor
Down
Sleep 300ms
Down
Sleep 300ms
Down
Sleep 1200ms

# Ctrl+E to export
Ctrl+E
Sleep 2500ms

# Show exported file path
Enter
Sleep 2000ms

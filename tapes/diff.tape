# diff.tape — claude-picker --diff two sessions side by side
# Run: vhs tapes/diff.tape

Output diff.gif
Output diff.webm

Set Shell bash
Set FontSize 14
Set Width 1400
Set Height 800
Set Theme "Catppuccin Mocha"
Set Padding 30
Set TypingSpeed 45ms
Set CursorBlink false

Sleep 500ms
Type "claude-picker --diff"
Sleep 600ms
Enter
Sleep 300ms
Type@0 "bash demo-mode-diff.sh"
Hide
Enter
Show
Sleep 6000ms

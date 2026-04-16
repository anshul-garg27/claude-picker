# diff.tape — claude-picker --diff two sessions side by side
# Run: cd ~/Desktop/claude-picker && vhs tapes/diff.tape

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

Hide
Type "alias claude-picker='bash ./demo-mode-diff.sh' ; clear"
Enter
Show

Sleep 500ms
Type "claude-picker --diff"
Sleep 600ms
Enter
Sleep 6500ms

# stats.tape — claude-picker --stats dashboard
# Run: cd ~/Desktop/claude-picker && vhs tapes/stats.tape

Output stats.gif
Output stats.webm

Set Shell bash
Set FontSize 15
Set Width 1200
Set Height 800
Set Theme "Catppuccin Mocha"
Set Padding 30
Set TypingSpeed 45ms
Set CursorBlink false

Hide
Type "alias claude-picker='bash ./demo-mode-stats.sh' ; clear"
Enter
Show

Sleep 500ms
Type "claude-picker --stats"
Sleep 600ms
Enter
Sleep 5000ms

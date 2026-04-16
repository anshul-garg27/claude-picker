# stats.tape — claude-picker --stats dashboard
# Run: vhs tapes/stats.tape

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

Sleep 500ms
Type "claude-picker --stats"
Sleep 600ms
Enter
Sleep 300ms
Type@0 "bash demo-mode-stats.sh"
Hide
Enter
Show
Sleep 4000ms

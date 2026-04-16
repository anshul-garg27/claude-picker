# tree.tape — claude-picker --tree with fork detection
# Run: cd ~/Desktop/claude-picker && vhs scripts/tapes/tree.tape

Output assets/gifs/tree.gif
Output assets/videos/tree.webm

Set Shell bash
Set FontSize 15
Set Width 1200
Set Height 800
Set Theme "Catppuccin Mocha"
Set Padding 30
Set TypingSpeed 45ms
Set CursorBlink false

Hide
Type "alias claude-picker='bash ./scripts/demo-mode-tree.sh' ; clear"
Enter
Show

Sleep 500ms
Type "claude-picker --tree"
Sleep 600ms
Enter
Sleep 5500ms

# hero.tape — main claude-picker flow
# Run: cd ~/Desktop/claude-picker && vhs scripts/tapes/hero.tape

Output assets/gifs/hero.gif
Output assets/videos/hero.webm

Set Shell bash
Set FontSize 16
Set Width 1400
Set Height 800
Set Theme "Catppuccin Mocha"
Set Padding 30
Set TypingSpeed 50ms
Set CursorBlink false

Hide
Type "alias claude-picker='bash ./scripts/demo-mode.sh' ; clear"
Enter
Show

Sleep 500ms
Type "claude-picker"
Sleep 800ms
Enter
Sleep 2500ms

# Project picker — hover options then select
Sleep 500ms
Down
Sleep 400ms
Up
Sleep 600ms
Enter
Sleep 2500ms

# Session picker — browse sessions
Sleep 500ms
Down
Sleep 300ms
Down
Sleep 300ms
Down
Sleep 1500ms

Down
Sleep 1500ms

Down
Sleep 1500ms

Up
Sleep 400ms
Up
Sleep 1500ms

Enter
Sleep 2000ms

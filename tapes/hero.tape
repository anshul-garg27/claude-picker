# hero.tape — main claude-picker flow
# Project picker → session picker → preview → resume
# Run: vhs tapes/hero.tape

Output hero.gif
Output hero.webm

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
Sleep 800ms
Enter
Sleep 2500ms

# Project picker — hover two options, then select architex
Sleep 500ms
Down
Sleep 400ms
Up
Sleep 600ms
Enter
Sleep 2500ms

# Session picker — browse down through named sessions
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

# Back up to auth-refactor
Up
Sleep 400ms
Up
Sleep 1500ms

# Resume
Enter
Sleep 2000ms

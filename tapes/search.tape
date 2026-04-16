# search.tape — claude-picker --search full-text across projects
# Run: vhs tapes/search.tape

Output search.gif
Output search.webm

Set Shell bash
Set FontSize 16
Set Width 1400
Set Height 800
Set Theme "Catppuccin Mocha"
Set Padding 30
Set TypingSpeed 50ms
Set CursorBlink false

Sleep 500ms
Type "claude-picker --search"
Sleep 600ms
Enter
Sleep 300ms
Type@0 "bash demo-mode-search.sh"
Hide
Enter
Show
Sleep 2500ms

# Browse results
Down
Sleep 700ms
Down
Sleep 700ms
Down
Sleep 1200ms
Up
Sleep 400ms
Up
Sleep 400ms
Up
Sleep 1000ms

# Select first result (auth-refactor in architex)
Enter
Sleep 2000ms

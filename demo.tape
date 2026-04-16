# claude-picker demo recording
# Run: cd ~/Desktop/claude-picker && vhs demo.tape

Output demo.gif
Output demo.webm

Set Shell bash
Set FontSize 16
Set Width 1400
Set Height 800
Set Theme "Catppuccin Mocha"
Set Padding 30
Set TypingSpeed 50ms
Set CursorBlink false

# Type the command
Sleep 500ms
Type "bash demo-mode.sh"
Sleep 800ms
Enter
Sleep 2500ms

# Step 1: Project picker — navigate and select architex
Sleep 500ms
Down
Sleep 400ms
Up
Sleep 600ms
Enter
Sleep 2500ms

# Step 2: Session picker — browse sessions
# Move down through New Session → saved header → auth-refactor
Sleep 500ms
Down
Sleep 300ms
Down
Sleep 300ms
Down
Sleep 1500ms

# Move to fix-race-condition (show preview changing)
Down
Sleep 1500ms

# Move to drizzle-migration
Down
Sleep 1500ms

# Go back up to auth-refactor
Up
Sleep 400ms
Up
Sleep 1500ms

# Select auth-refactor
Enter
Sleep 2000ms

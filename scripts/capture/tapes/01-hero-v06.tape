# claude-picker v0.6 hero capture
# Shows: Kanagawa default, fuzzy filter, audit with annual-savings + per-project bar,
# stats with burn-rate + project-cost 30d heatmap, conversation viewer with
# interesting-moments timeline + subagent tree.
#
# Render pipeline:
#   cd scripts/capture/out
#   vhs ../tapes/01-hero-v06.tape                 # emits hero-v06.gif + frames/
#   gifski -o hero-v06-hq.gif --fps 24 --quality 100 frames/frame-*.png
#   cp hero-v06-hq.gif ../../../code/assets/hero.gif
#
# Tuned against the v0.4.0+ VHS docs — WindowBar, BorderRadius, larger
# FontSize, and both GIF + frames/ outputs for the gifski re-encode pass.

Output hero-v06.gif
Output hero-v06.mp4
Output frames/

Set Shell "bash"
Set FontFamily "JetBrains Mono"
# 18pt @ 1920 ≈ 102 cols × 36 rows — above the 80×20 minimum the picker
# enforces (see conversation_viewer MIN_VIEWER_WIDTH = 60 + picker gate
# in app.rs). Landing at 18pt keeps the glyphs crisp on retina while
# giving audit / stats enough horizontal space for the by-project band.
Set FontSize 18
Set Width 1920
Set Height 1200
Set Padding 40
Set Margin 50
Set MarginFill "#1F1F28"
Set WindowBar Colorful
Set WindowBarSize 40
Set BorderRadius 12
Set TypingSpeed 55ms
Set PlaybackSpeed 1.0

# Kanagawa inline — mirrors src/theme.rs so the capture matches the
# in-binary default exactly even if VHS doesn't know the theme name.
Set Theme { "name": "Kanagawa", "background": "#1F1F28", "foreground": "#DCD7BA", "black": "#090618", "red": "#C34043", "green": "#76946A", "yellow": "#C0A36E", "blue": "#7E9CD8", "purple": "#957FB8", "cyan": "#6A9589", "white": "#C8C093", "brightBlack": "#727169", "brightRed": "#E82424", "brightGreen": "#98BB6C", "brightYellow": "#E6C384", "brightBlue": "#7FB4CA", "brightPurple": "#938AA9", "brightCyan": "#7AA89F", "brightWhite": "#DCD7BA", "cursor": "#C8C093", "cursorAccent": "#1F1F28", "selectionBackground": "#2D4F67" }

Hide
Type "export PS1='$ '"
Enter
Type "clear"
Enter
Show

# Act 1 — land on the picker (Kanagawa default)
Sleep 600ms
Type "claude-picker"
Sleep 400ms
Enter
Sleep 2500ms

# Browse a project, fuzzy filter a little
Down
Sleep 500ms
Down
Sleep 500ms
Type "aud"
Sleep 1s
Backspace 3
Sleep 400ms

# Open the project → session list — shows chain badges + anomaly badges
Enter
Sleep 2500ms

Down
Sleep 400ms
Down
Sleep 400ms
Up
Sleep 600ms

# Act 2 — quit back to shell, then open the audit subcommand directly.
# A single `q` exits the picker entirely (App::should_quit = true), so we
# don't chain multiple q's — stray keystrokes would land in the bash buffer
# and corrupt the next command. Sleep generously to let the TTY restore.
Type "q"
Sleep 900ms
Type "clear"
Enter
Sleep 400ms

Type "claude-picker audit"
Sleep 300ms
Enter
Sleep 3500ms

# Scroll a bit in the audit list so the summary band + per-project bar
# stay visible but the user sees a drill-in possible
Down
Sleep 400ms
Down
Sleep 400ms
Sleep 1500ms

Type "q"
Sleep 900ms
Type "clear"
Enter
Sleep 400ms

# Act 3 — open stats dashboard (burn-rate + 7×24 patterns + project heatmap)
Type "claude-picker stats"
Sleep 300ms
Enter
Sleep 3500ms

# Cycle the pattern heatmap metric to show the 'p' binding works
Type "p"
Sleep 1200ms
Type "p"
Sleep 1200ms

Type "q"
Sleep 900ms
Type "clear"
Enter
Sleep 400ms

# Act 4 — one-liner scripting: shell-prompt spend summary
Type "claude-picker prompt"
Sleep 300ms
Enter
Sleep 2500ms

# Final frame holds long enough for the eye to rest before the loop
Sleep 2500ms

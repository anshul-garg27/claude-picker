# Demo tapes

Each `.tape` file is a [VHS](https://github.com/charmbracelet/vhs) script that records a specific claude-picker feature as a GIF + WebM.

## Prerequisites

```bash
brew install vhs
```

## Record a single demo

```bash
cd ~/Desktop/claude-picker
vhs tapes/hero.tape        # main flow — project → session → preview → resume
vhs tapes/search.tape      # --search across all projects
vhs tapes/stats.tape       # --stats dashboard
vhs tapes/tree.tape        # --tree with forks
vhs tapes/diff.tape        # --diff two sessions
vhs tapes/bookmarks.tape   # Ctrl+B pin a session
vhs tapes/export.tape      # Ctrl+E export to markdown
```

## Record everything

```bash
cd ~/Desktop/claude-picker/tapes
for t in *.tape; do vhs "$t"; done
```

## Output

Every tape writes both a `.gif` (for README, Twitter, Reddit) and a `.webm` (for Medium, Dev.to, LinkedIn). Files land in the project root so you can reference them with relative paths.

## How they work

The hero, bookmarks, and export tapes run `./demo-mode.sh` — a hardcoded flow so the recording looks identical on every machine. Stats, tree, search, and diff tapes use their own `demo-mode-<feature>.sh` scripts for the same reason.

If you want to record against your real session data instead, replace `bash ./demo-mode.sh` with `claude-picker` inside the tape file.

## File sizes

Target sizes for each GIF:

| tape | duration | dimensions | target size |
|------|----------|------------|-------------|
| hero.tape | ~15s | 1400x800 | < 4 MB |
| search.tape | ~10s | 1400x800 | < 3 MB |
| stats.tape | ~8s | 1200x800 | < 2 MB |
| tree.tape | ~8s | 1200x800 | < 2 MB |
| diff.tape | ~12s | 1400x800 | < 3 MB |
| bookmarks.tape | ~10s | 1400x800 | < 3 MB |
| export.tape | ~8s | 1400x800 | < 2 MB |

If a GIF exceeds its target, post-process:

```bash
gifski --fps 12 --width 800 --quality 80 -o out.gif recording.mov
# or
ffmpeg -i input.gif -vf "fps=12,scale=800:-1:flags=lanczos" -loop 0 output.gif
```

# scripts/

Demo data generators and vhs recording scripts. None of these ship with the tool — they exist only for making marketing assets.

## What's here

| Script | Used by | Output |
|--------|---------|--------|
| `demo-mode.sh` | `tapes/hero.tape`, `tapes/bookmarks.tape`, `tapes/export.tape` | Interactive fzf demo of project → session flow |
| `demo-mode-stats.sh` | `tapes/stats.tape` + `assets/generate.sh` | `--stats` dashboard printout |
| `demo-mode-tree.sh` | `tapes/tree.tape` + `assets/generate.sh` | `--tree` with fork connectors |
| `demo-mode-search.sh` | `tapes/search.tape` | Interactive fzf demo of `--search` |
| `demo-mode-diff.sh` | `tapes/diff.tape` + `assets/generate.sh` | Side-by-side diff view |
| `demo-mockup-before.sh` | `assets/generate.sh` | Static "claude --resume" problem view |
| `demo-mockup-projects.sh` | `assets/generate.sh` | Static project picker view |
| `demo-mockup-sessions.sh` | `assets/generate.sh` | Static session picker with preview |
| `tapes/*.tape` | `vhs` | Feature GIFs + WebMs into `assets/gifs/` and `assets/videos/` |

## Why hardcoded demo data?

Every marketing GIF needs to look **identical** across machines. Running against real user sessions would produce different-looking recordings depending on what conversations you've had.

Each `.tape` file sets `alias claude-picker='bash ./scripts/demo-mode-X.sh'` inside a Hide block, so the GIF shows `claude-picker --stats` being typed while the demo script runs for reproducible output.

## Regenerating everything

```bash
# From project root
for t in scripts/tapes/*.tape; do vhs "$t"; done   # GIFs + WebMs
bash assets/generate.sh                             # mockups
bash assets/extract-frames.sh                       # frame extracts
```

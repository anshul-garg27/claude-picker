# assets/

All visual deliverables for claude-picker.

## What's here

| Folder | Contents | How they're made |
|--------|----------|------------------|
| `gifs/` | 7 animated GIFs (hero, search, stats, tree, diff, bookmarks, export) | `vhs scripts/tapes/*.tape` |
| `videos/` | 7 WebM files (same features) | Also from vhs |
| `mockups/` | 6 PNG + 6 SVG (freeze-generated, print-ready at any size) | `bash assets/generate.sh` |
| `frames/` | 15 PNG stills extracted from the 7 GIFs | `bash assets/extract-frames.sh` |
| `ai-generated/` | Future Gemini/DALL-E outputs, organised per platform | Manual — see prompts in `content/image-prompts.md` |
| `generate.sh` | Re-build all mockups with freeze | — |
| `extract-frames.sh` | Re-extract frames from GIFs with ffmpeg | — |

## Where each asset gets used

See `content/USAGE.md` for the complete platform-by-platform map.

## Required tools

```bash
brew install vhs                           # for GIFs
brew install charmbracelet/tap/freeze      # for mockup PNG/SVG
brew install ffmpeg                        # for frame extraction
```

# AI-generated images

This folder holds the images you create using the Gemini / DALL-E prompts in `content/image-prompts.md` and `content/instagram-linkedin.md`.

Everything else in `assets/` is generated from code (`gifs/`, `videos/`, `mockups/`, `frames/`). Only this folder is for manually-generated AI art.

## Folder layout

```
ai-generated/
├── github/          → social preview, logo, favicon
├── medium/          → hero, architecture diagram, inline feature shots
├── twitter/         → card variants, feature highlight cards
├── reddit/          → thumbnail, post image
├── linkedin/        → carousel slides (12), OG image
├── instagram/       → stories (10)
└── producthunt/     → thumbnail, gallery (4–7), promo video
```

## Where each prompt saves to

See the full table in `content/USAGE.md` → "AI-generated images" section.

Short version:
- Prompts 1–25 in `image-prompts.md` each say exactly where the file should be saved.
- Instagram stories prompts (inside `instagram-linkedin.md`) → `instagram/story-01.png` to `story-10.png`
- LinkedIn carousel prompts (inside `instagram-linkedin.md`) → `linkedin/slide-01.png` to `slide-12.png`

## Naming convention

Lowercase kebab-case. Descriptive. Examples:
- `social-preview.png`
- `hero.png`
- `slide-06-cost.png`
- `story-04-cost.png`
- `gallery-02.png`

## Don't commit raw Gemini output

Rename the file before committing. Files named `Gemini_Generated_Image_xxxx.png` are noise and hard to find later.

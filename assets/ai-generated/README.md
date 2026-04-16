# AI-generated images

Optional social-sharing images (GitHub social preview, logo, favicon, platform-specific cards). These are manually generated — the repo doesn't depend on them.

## Folder layout

```
ai-generated/
├── github/          → social preview, logo, favicon
├── medium/          → hero, architecture diagram, inline feature shots
├── twitter/         → card variants, feature highlight cards
├── reddit/          → thumbnail, post image
├── linkedin/        → carousel slides, OG image
├── instagram/       → stories
└── producthunt/     → thumbnail, gallery, promo video
```

## Naming convention

Lowercase kebab-case. Examples:
- `social-preview.png`
- `hero.png`
- `slide-06-cost.png`
- `gallery-02.png`

Rename files before committing — don't leave raw generator output like `Gemini_Generated_Image_xxxx.png`.

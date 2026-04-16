# Asset Usage Guide

Complete map of **which file goes where** for the claude-picker launch.

- [Folder structure](#folder-structure)
- [Per-platform placement](#per-platform-placement) — the main reference
- [AI-generated images](#ai-generated-images) — where Gemini outputs go
- [Regenerating assets](#regenerating-assets)

---

## Folder structure

```
claude-picker/
├── README.md                          # the only README users see first
├── content/                           # marketing copy (text only)
│   ├── medium-article.md              # full blog post
│   ├── twitter-thread.md              # 12-tweet thread
│   ├── reddit-hackernews-warp.md      # platform-specific posts
│   ├── instagram-linkedin.md          # stories + carousel + prompts
│   ├── launch-playbook.md             # day-by-day launch schedule + PH
│   ├── image-prompts.md               # 25 Gemini/DALL-E prompts
│   └── USAGE.md                       # (this file)
│
├── assets/                            # all visual deliverables
│   ├── gifs/                          # 7 animated GIFs (for README, Twitter, Reddit)
│   ├── videos/                        # 7 WebM files (for Medium, Dev.to, LinkedIn)
│   ├── mockups/                       # 6 PNG + 6 SVG (freeze-generated, any size)
│   ├── frames/                        # 15 PNG (ffmpeg-extracted from GIFs)
│   ├── ai-generated/                  # Gemini/DALL-E outputs per platform
│   │   ├── github/
│   │   ├── medium/
│   │   ├── twitter/
│   │   ├── reddit/
│   │   ├── linkedin/
│   │   ├── instagram/
│   │   └── producthunt/
│   ├── generate.sh                    # rebuild all mockups
│   └── extract-frames.sh              # re-extract frames from GIFs
│
└── scripts/                           # demo data + vhs recording scripts
    ├── demo-mode.sh                   # used by hero/bookmarks/export tapes
    ├── demo-mode-stats.sh             # used by stats.tape + stats mockup
    ├── demo-mode-tree.sh              # used by tree.tape + tree mockup
    ├── demo-mode-search.sh            # used by search.tape
    ├── demo-mode-diff.sh              # used by diff.tape + diff mockup
    ├── demo-mockup-before.sh          # static "before claude-picker" image
    ├── demo-mockup-projects.sh        # static project picker image
    ├── demo-mockup-sessions.sh        # static session+preview image
    └── tapes/                         # .tape files for vhs
```

---

## Per-platform placement

### GitHub — the repository itself

| Asset | Path | Where it goes | Size target |
|-------|------|---------------|-------------|
| Hero GIF | `assets/gifs/hero.gif` | README.md, above-the-fold | < 4 MB |
| Feature GIFs | `assets/gifs/*.gif` | README.md, inside `<details>` | < 3 MB each |
| Social preview | `assets/ai-generated/github/social-preview.png` | Repo Settings → Social preview | **1280x640** |

**How to set the social preview:** GitHub repo → Settings → scroll to "Social preview" → upload the 1280x640 PNG generated from prompt #1 in `image-prompts.md`.

### Medium article

The long-form blog post. Needs a hero, inline screenshots, and maybe a WebM or two.

| Asset | Path | Where in the article |
|-------|------|---------------------|
| Hero image | `assets/ai-generated/medium/hero.png` (1400x788) | Very top, before the subtitle |
| Before/after split | `assets/ai-generated/medium/before-after.png` (1200x600) | After "Here's what the built-in picker shows" |
| Project picker still | `assets/mockups/projects.png` or `assets/frames/hero-01-projects.png` | "Pick a project" paragraph |
| Session picker still | `assets/mockups/sessions.png` or `assets/frames/hero-02-sessions.png` | "Pick a session" paragraph |
| Stats mockup | `assets/mockups/stats.png` | `--stats` section |
| Tree mockup | `assets/mockups/tree.png` | `--tree` section |
| Diff mockup | `assets/mockups/diff.png` | `--diff` section |
| Architecture diagram | `assets/ai-generated/medium/architecture.png` (1200x800) | "How the pieces fit together" |
| Hero WebM (optional) | `assets/videos/hero.webm` | Can be embedded as a video |

Medium accepts PNG/JPG/GIF natively. For WebM, upload as a video via the `+` menu.

### Twitter / X — the 12-tweet thread

One tweet = one image, max 4 images per tweet. See `content/twitter-thread.md` for tweet text.

| Tweet | Asset | Path |
|-------|-------|------|
| 1 — hook | Hero GIF | `assets/gifs/hero.gif` |
| 2 — problem | Before mockup | `assets/mockups/before.png` |
| 3 — solution | Project picker | `assets/mockups/projects.png` |
| 4 — `--search` | Search GIF | `assets/gifs/search.gif` |
| 5 — `--stats` | Stats mockup | `assets/mockups/stats.png` |
| 6 — `--tree` | Tree mockup | `assets/mockups/tree.png` |
| 7 — `--diff` | Diff mockup | `assets/mockups/diff.png` |
| 8 — bookmarks | Bookmarks GIF | `assets/gifs/bookmarks.gif` |
| 9 — keyboard shortcuts | Feature card | `assets/ai-generated/twitter/shortcuts-card.png` (1200x675) |
| 10 — Claude Code skill | Feature card | `assets/ai-generated/twitter/skill-card.png` (1200x675) |
| 11 — insight (naming) | Feature card | `assets/ai-generated/twitter/naming-card.png` (1200x675) |
| 12 — CTA | Twitter card | `assets/ai-generated/twitter/cta-card.png` (1200x675) |

**Rule:** Tweet 1 MUST have the hero GIF. Engagement drops 3–5x without it.

### Reddit — posts and thumbnails

One image per post. Must be readable as a 70x70 thumbnail.

| Subreddit | Post in | Asset |
|-----------|---------|-------|
| r/ClaudeAI | Top of body | `assets/gifs/hero.gif` |
| r/commandline | Top of body | `assets/mockups/sessions.png` (shows fzf craft) |
| r/terminal | Top of body | `assets/mockups/projects.png` (UI forward) |
| Any | Thumbnail variant | `assets/ai-generated/reddit/thumbnail.png` (1200x630) |

### Hacker News Show HN

HN strips images from post bodies — the only image is the thumbnail shown next to the post URL. Point the HN URL at your GitHub repo so the repo's **social preview** becomes the thumbnail.

| Action | Asset | Note |
|--------|-------|------|
| Set GitHub social preview | `assets/ai-generated/github/social-preview.png` | Do this *before* submitting to HN |
| Founder comment (optional GIF link) | `assets/gifs/hero.gif` hosted on GitHub | Link to the raw URL in your first comment |

### LinkedIn

Two separate posts per `launch-playbook.md`: a text post and a carousel PDF.

**Text post:**
| Asset | Path | Size |
|-------|------|------|
| OG image (auto-generated from link) | set via GitHub social preview | 1200x630 |
| Optional inline image | `assets/mockups/stats.png` | any |

**Carousel PDF (12 slides):**
Generate 12 slides at 1080x1350 each using the prompts in `instagram-linkedin.md`, then combine into one PDF.

Save them here:
```
assets/ai-generated/linkedin/
├── slide-01-cover.png
├── slide-02-problem.png
├── slide-03-solution.png
├── slide-04-step1.png
├── slide-05-step2.png
├── slide-06-cost.png
├── slide-07-stats.png
├── slide-08-tree.png
├── slide-09-search.png
├── slide-10-diff.png
├── slide-11-features.png
├── slide-12-cta.png
└── carousel.pdf              ← combine the 12 PNGs into this
```

**PDF tip:** on macOS, open all 12 PNGs in Preview in order, then `File → Print → Save as PDF`. Or use:
```bash
cd assets/ai-generated/linkedin
magick slide-*.png carousel.pdf   # ImageMagick
```

### Instagram — 10 Stories

Stories are 1080x1920 (9:16). All text-on-dark, no photos.

Save the 10 story slides here:
```
assets/ai-generated/instagram/
├── story-01-hook.png
├── story-02-problem.png
├── story-03-intro.png
├── story-04-cost.png
├── story-05-how-it-works.png
├── story-06-stats.png
├── story-07-tree.png
├── story-08-search.png
├── story-09-shortcuts.png
└── story-10-cta.png
```

Prompts are in `instagram-linkedin.md`. Post all 10 stories in one sitting — they form a narrative.

### Product Hunt

PH requires a thumbnail, a gallery of 4–7 images, and an optional promo video.

Save here:
```
assets/ai-generated/producthunt/
├── thumbnail.png          # 240x240 — the logo square
├── gallery-01.png         # 1270x760 — hero/main screenshot
├── gallery-02.png         # 1270x760 — feature showcase
├── gallery-03.png         # 1270x760 — another angle
├── gallery-04.png         # 1270x760 — cost tracking highlight
├── gallery-05.png         # 1270x760 — comparison/stats
└── promo.mp4              # OPTIONAL — 30–60 second video
```

You can also use existing assets directly:
- Gallery slot 1 → `assets/frames/hero-02-sessions.png` (resize to 1270x760)
- Gallery slot 2 → `assets/mockups/stats.png`
- Gallery slot 3 → `assets/mockups/tree.png`
- Gallery slot 4 → `assets/mockups/diff.png`

Prompts for the generated ones are in `image-prompts.md` (Images #19 and #20).

### Warp Community / Discord

| Asset | Path |
|-------|------|
| Inline screenshot | `assets/mockups/sessions.png` |
| GIF link | `assets/gifs/hero.gif` on GitHub |

---

## AI-generated images

These are the images you'll create using the prompts in `image-prompts.md`. They live in `assets/ai-generated/<platform>/`.

### Workflow

1. **Copy prompt** from `image-prompts.md` (there are 25 numbered prompts)
2. **Paste** into Gemini AI Pro (primary), DALL-E 3 (fallback), or Flux.1 Pro (for text-heavy images)
3. **Download** the generated PNG
4. **Rename** using the naming convention below
5. **Save** into the matching `assets/ai-generated/<platform>/` folder

### Naming convention

Use lowercase kebab-case that describes the content:

```
<purpose>.png
<platform-specific-prefix>-<number>-<topic>.png
```

Examples:
- `social-preview.png` (GitHub)
- `hero.png` (Medium)
- `slide-06-cost.png` (LinkedIn)
- `story-04-cost.png` (Instagram)
- `gallery-02.png` (Product Hunt)
- `shortcuts-card.png` (Twitter)

### Image-to-folder map

This table tells you which prompt number produces which file, and where it lives:

| Prompt # in image-prompts.md | Purpose | Save as |
|------------------------------|---------|---------|
| 1 | GitHub Social Preview | `assets/ai-generated/github/social-preview.png` |
| 2 | Medium Hero | `assets/ai-generated/medium/hero.png` |
| 3 | Before vs After comparison | `assets/ai-generated/medium/before-after.png` |
| 4 | Architecture Diagram | `assets/ai-generated/medium/architecture.png` |
| 5 | Open Graph / LinkedIn share | `assets/ai-generated/linkedin/og-image.png` |
| 6 | Project picker (styled) | `assets/ai-generated/medium/project-picker.png` |
| 7 | Session picker + preview (styled) | `assets/ai-generated/medium/session-preview.png` |
| 8 | `--search` showcase | `assets/ai-generated/medium/feature-search.png` |
| 9 | `--stats` showcase | `assets/ai-generated/medium/feature-stats.png` |
| 10 | `--tree` showcase | `assets/ai-generated/medium/feature-tree.png` |
| 11 | `--diff` showcase | `assets/ai-generated/medium/feature-diff.png` |
| 12 | Bookmarks highlight | `assets/ai-generated/twitter/bookmarks-card.png` |
| 13 | Export highlight | `assets/ai-generated/twitter/export-card.png` |
| 14 | Cost tracking highlight | `assets/ai-generated/twitter/cost-card.png` |
| 15 | Logo 512x512 | `assets/ai-generated/github/logo.png` |
| 16 | Favicon 64x64 | `assets/ai-generated/github/favicon.png` |
| 17 | Twitter Card generic | `assets/ai-generated/twitter/card-generic.png` |
| 18 | Reddit thumbnail | `assets/ai-generated/reddit/thumbnail.png` |
| 19 | Product Hunt gallery | `assets/ai-generated/producthunt/gallery-01.png` |
| 20 | Product Hunt thumbnail | `assets/ai-generated/producthunt/thumbnail.png` |
| 21 | Instagram Story template | `assets/ai-generated/instagram/story-template.png` |
| 22 | LinkedIn carousel template | `assets/ai-generated/linkedin/slide-template.png` |
| 23 | Claude Code skill card | `assets/ai-generated/twitter/skill-card.png` |
| 24 | Warp integration card | `assets/ai-generated/twitter/warp-card.png` |
| 25 | Age warning color key | `assets/ai-generated/medium/age-warnings.png` |

### Instagram stories (10 slides from `instagram-linkedin.md`)

Personal, first-person story arc — not a feature pitch. Each slide is a chapter in the "I was building something → found a problem → built a fix" journey. Post all 10 in one sitting so the story reads end-to-end.

| Slide | Story beat | Save as |
|-------|------------|---------|
| 1 | Context — "been using claude code every day" | `assets/ai-generated/instagram/story-01-where-it-started.png` |
| 2 | The moment — `claude --resume` shows UUIDs | `assets/ai-generated/instagram/story-02-the-moment.png` |
| 3 | The vent — "four wrong clicks" | `assets/ai-generated/instagram/story-03-four-wrong-clicks.png` |
| 4 | Curiosity — "opened ~/.claude/" | `assets/ai-generated/instagram/story-04-got-curious.png` |
| 5 | Discovery — JSONL files, one per session | `assets/ai-generated/instagram/story-05-what-i-found.png` |
| 6 | Building — "two hours later..." | `assets/ai-generated/instagram/story-06-started-building.png` |
| 7 | Stats — "more on one project than on lunch" | `assets/ai-generated/instagram/story-07-stats.png` |
| 8 | Search — "grep, but for conversations" | `assets/ai-generated/instagram/story-08-search.png` |
| 9 | Daily use — "20 times a day" + naming habit | `assets/ai-generated/instagram/story-09-how-i-use-it.png` |
| 10 | Share — "lmk what breaks" + repo link | `assets/ai-generated/instagram/story-10-share.png` |

### LinkedIn carousel (12 slides from `instagram-linkedin.md`)

| Slide | Prompt section | Save as |
|-------|---------------|---------|
| Cover | Carousel Slide 1 | `assets/ai-generated/linkedin/slide-01-cover.png` |
| Problem | Carousel Slide 2 | `assets/ai-generated/linkedin/slide-02-problem.png` |
| Solution | Carousel Slide 3 | `assets/ai-generated/linkedin/slide-03-solution.png` |
| Step 1 | Carousel Slide 4 | `assets/ai-generated/linkedin/slide-04-step1.png` |
| Step 2 | Carousel Slide 5 | `assets/ai-generated/linkedin/slide-05-step2.png` |
| Cost | Carousel Slide 6 | `assets/ai-generated/linkedin/slide-06-cost.png` |
| `--stats` | Carousel Slide 7 | `assets/ai-generated/linkedin/slide-07-stats.png` |
| `--tree` | Carousel Slide 8 | `assets/ai-generated/linkedin/slide-08-tree.png` |
| `--search` | Carousel Slide 9 | `assets/ai-generated/linkedin/slide-09-search.png` |
| `--diff` | Carousel Slide 10 | `assets/ai-generated/linkedin/slide-10-diff.png` |
| Features | Carousel Slide 11 | `assets/ai-generated/linkedin/slide-11-features.png` |
| CTA | Carousel Slide 12 | `assets/ai-generated/linkedin/slide-12-cta.png` |

---

## Regenerating assets

### Re-record all GIFs

```bash
cd ~/Desktop/claude-picker
for t in scripts/tapes/*.tape; do vhs "$t"; done
```

### Re-generate all static mockups

```bash
bash assets/generate.sh
```

### Re-extract frames from GIFs

```bash
bash assets/extract-frames.sh
```

### Tools required

| Tool | Install | Purpose |
|------|---------|---------|
| `vhs` | `brew install vhs` | GIF + WebM from tape files |
| `freeze` | `brew install charmbracelet/tap/freeze` | PNG + SVG from command output |
| `ffmpeg` | `brew install ffmpeg` | Frame extraction from GIFs |

---

## Quick reference — what to grab for each platform

If you're in a rush, here's the minimum viable asset set per platform:

| Platform | Minimum | Nice to have |
|----------|---------|--------------|
| GitHub README | `assets/gifs/hero.gif` | + social preview PNG |
| Medium | Hero image + 3 mockups | All feature mockups |
| Twitter tweet 1 | `assets/gifs/hero.gif` | 11 more images for the thread |
| Reddit post | `assets/gifs/hero.gif` | + one mockup |
| HN | Repo social preview set | — |
| LinkedIn text | One mockup | — |
| LinkedIn carousel | 12 PNGs as PDF | — |
| Instagram | 10 story PNGs | — |
| Product Hunt | Thumbnail + 4 gallery images | + promo MP4 |

Ship the essentials. Fill in the rest later.

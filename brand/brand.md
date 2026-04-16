# claude-picker Brand Guide

The official identity kit for **claude-picker**. If you are shipping something that represents the project — a website, a README header, a social profile, a package listing, a talk slide — use this guide.

---

## The mark — "The 4-Dot Picker"

The primary mark is a **2 x 2 grid of four dots**:

```
    [ ]   [ ]      <- unselected sessions
    [ ]    o       <- one circle (selected, the cursor)
```

- **Three rounded squares** — unselected sessions in the picker list.
- **One circle (bottom-right)** — the currently selected session. The cursor.

It is a miniature of the product itself: a picker with four rows, one highlighted. The shape difference between square and circle does the work of a highlight color, which means the mark stays one-ink and still encodes selection.

Three traits make this mark work:

1. **One concept, literally.** The logo is a picker view. Every dot means a session; the circle means "selected".
2. **Monochromatic, geometric, stroke-free.** Every asset is a single fill color. No gradients, no glows, no outlines.
3. **Survives at 16 x 16.** Four pixels of difference are enough for the circle to still read as not-a-square.

---

## Color palette

| Role | Name | Hex | Notes |
|------|------|------|-------|
| Primary | Catppuccin Mauve | `#CBA6F7` | The only color the mark is ever drawn in, except the mono variant |
| Background (dark) | Catppuccin Base | `#1E1E2E` | Default backdrop for the mauve mark |
| Mono fill | Catppuccin Base | `#1E1E2E` | Used when the mark must be printed in a single dark ink on light |
| Text (dark bg) | Catppuccin Text | `#CDD6F4` | Body copy companion — not part of the mark |
| Text (light bg) | Catppuccin Base | `#1E1E2E` | Body copy companion on light |

No other colors. Do not introduce teals, reds, or brand accents. One glance should tell you this is from the Catppuccin-flavored CLI family.

---

## Typography

| Context | Font | Weight | Usage |
|---------|------|--------|-------|
| Wordmark | JetBrains Mono | Bold (700) | `claude-picker` lockup in headers, hero, social |
| Code blocks / CLI snippets | JetBrains Mono | Regular (400) | README code fences, website docs, terminal mockups |
| UI / prose | Inter or system-ui | Regular / Semibold | Website body copy only. Never used in the mark. |

JetBrains Mono is the brand's typographic signature because it is the native typeface of the terminal context the product lives in. Never substitute a proportional font in the wordmark — the monospaced rhythm is the point.

If a downstream consumer cannot embed JetBrains Mono, outline the text from `wordmark.svg` via Figma / Illustrator "Convert to outlines" before export. Do not substitute a different mono.

---

## Geometry spec (canonical)

On a 64 x 64 viewBox:

| Element | Position (x, y) | Size | Radius |
|---------|-----------------|------|--------|
| Top-left square | (6, 6) | 24 x 24 | 6 |
| Top-right square | (34, 6) | 24 x 24 | 6 |
| Bottom-left square | (6, 34) | 24 x 24 | 6 |
| Bottom-right circle | cx 46, cy 46 | r = 10 (diameter 20) | — |
| Gap between dots | 4u on both axes | | |
| Edge clearspace | 6u on all sides | | |
| Grid footprint | 52 x 52u | | |

**Why the circle is smaller than the squares.** A 20-diameter circle against 24-side squares has roughly equal visual weight (the square occupies more pixel area, which a circle's roundness offsets). More importantly, the size difference reinforces that the circle is *different* — not just a rotated square. This is the same trick Google uses with the "o" in its new logo versus the other letterforms.

---

## Sizes & minimums

| Asset | Minimum | Typical use |
|-------|---------|-------------|
| `favicon.svg` | 16 px | Browser tab, iOS home-screen |
| `logo.svg` in-line | 24 px | Mini badges, footer icons |
| `logo.svg` header | 48 px | Nav bars, README header |
| `logo.svg` marketing | 128 px+ | Hero section, OG images |
| `wordmark.svg` | 180 px wide | Site header, footer, README hero |
| `logo-on-dark.svg` | 128 px wide | Social preview tiles |

Below the minimums the mark begins to lose fidelity. Do not scale below.

---

## Clearspace

Reserve clearspace equal to **1 x the glyph's height** (64u at canonical scale) on all sides of the primary mark.

```
     ┌──────────────────────────┐
     │                          │
     │   X          X           │
     │     ┌───────────┐        │
     │     │  [ ] [ ]  │        │
     │  X  │  [ ]  o   │  X     │
     │     └───────────┘        │
     │   X          X           │
     │                          │
     └──────────────────────────┘

     X = 1 x glyph height (= 64u at canonical)
```

No other graphic element (photography, heading, rule) may enter this zone.

---

## Variants

| File | Use |
|------|-----|
| `logo.svg` | Primary mauve mark on dark. README header, website nav, Product Hunt. |
| `logo-on-dark.svg` | Pre-composited 128 x 128 dark tile. GitHub avatar, LinkedIn share. |
| `logo-mono.svg` | Light backgrounds, single-ink print, rubber stamps. |
| `wordmark.svg` | Brand name explicit: site header, footer, hero, launch announcement. |
| `favicon.svg` | Browser tab. Tuned for 16 x 16 crispness. |
| `logo-alt-chevron.svg` | **Secondary motif only.** See "Alternate mark" below. |

---

## Wordmark lockup

Glyph on the left, `claude-picker` in JetBrains Mono Bold on the right, cap-height-aligned to the glyph's top and bottom edges.

- Gap between glyph and wordmark: **0.5 x glyph width** (= 32u on the 64u grid).
- Baseline shift: wordmark baseline at y = 52 so caps land flush at y = 6.
- Letter-spacing: -1.5 at font-size 72u (tightens JetBrains Mono's default airy tracking without breaking legibility).

Do not stack the wordmark below the glyph. Do not reduce the gap below 0.5x. Do not pair the glyph with any other wordmark (no "Claude Picker" with spaces, no smallcaps).

---

## Per-surface mapping

| Surface | File(s) | Notes |
|---------|---------|-------|
| GitHub repo avatar | `exports/logo-512.png` (from `logo.svg` on transparent) or `logo-on-dark.svg` rendered at 512px | Transparent preferred so GH renders it against whatever theme |
| GitHub README header | `wordmark.svg` | Embed as `<img src="brand/wordmark.svg" />` |
| GitHub social preview (1280 x 640) | `logo-on-dark.svg` centered, wordmark below | Keep OG-safe zone 120u from edges |
| Favicon (browser tab) | `favicon.svg` + `exports/logo-16.png`, `logo-32.png`, `logo-64.png` into `favicon.ico` | See *Regenerating favicon.ico* below |
| Website header | `wordmark.svg`, 180 px wide | 16 px clearspace |
| Website footer | `wordmark.svg`, 140 px wide | 32 px above |
| Website hero | `logo.svg` at 96–128 px above the H1 | |
| Open Graph image (1200 x 630) | `logo-on-dark.svg` centered + wordmark below in JetBrains Mono | Website-upgrade agent owns the composed PNG |
| Product Hunt thumbnail (240 x 240) | `exports/logo-240.png` over `#1E1E2E` | |
| Product Hunt gallery | Full composed card (see `assets/ai-generated/producthunt/`) | Already uses 4-dot mark |
| LinkedIn slides | Existing AI-generated carousel already uses the 4-dot silhouette | Do not regenerate |
| Instagram story | `assets/ai-generated/instagram/story-template.png` | Already uses 4-dot mark |
| Twitter OG | `logo-on-dark.svg` composed into 1200 x 630 PNG | |
| npm avatar | Same as GitHub avatar | |

---

## Do

- Use the mauve mark on dark backgrounds.
- Use the mono mark on light or single-ink contexts.
- Keep the circle in the **bottom-right** cell. Always.
- Keep 1 x clearspace around the mark.
- Scale proportionally.
- Pair with JetBrains Mono.

## Don't

- **Don't rotate.** The 2 x 2 has a fixed top-left / bottom-right orientation because "bottom-right = selected" reads left-to-right like a list.
- **Don't recolor.** No pink, no blue, no rainbow, no gradient — ever.
- **Don't add a fifth dot.** Four is the concept. Five breaks it.
- **Don't swap the circle to a square or a square to a circle.** The selected/unselected contrast is the logo's entire idea.
- **Don't rearrange the 2 x 2 to 1 x 4, 4 x 1, or diagonal.** The grid reads as "picker view" only when it's square.
- **Don't stretch, skew, or apply perspective.**
- **Don't add drop shadows, glows, bevels, strokes, or 3D.**
- **Don't place on a busy photographic background** without a `#1E1E2E` plate underneath.
- **Don't substitute a different monospace** for the wordmark.
- **Don't create derivative marks** that riff on Anthropic's Claude logo.
- **Don't interchange with the alternate chevron mark.** That one has a separate lane (see below).

---

## Alternate mark — "The Picker Caret"

Preserved at `brand/logo-alt-chevron.svg`.

A filled right-pointing chevron next to a rounded pill — the original design pass, representing the fzf cursor landing on a selected row. It has one permitted use lane: **secondary motifs for merch, interior branding, slide dividers, and CLI-native contexts** where a single line-of-text feel fits better than the grid.

Rules:

- **Never** appears in the same composition as the primary 4-dot mark.
- **Never** used as a favicon, avatar, or OG hero.
- **Never** in the website nav or footer.
- Allowed on: stickers, T-shirts, hoodie back prints, loading spinners (as a static chevron), CLI `--version` banners.

The two marks are **not** interchangeable. If a surface calls for a logo, it is the 4-dot mark.

---

## Export checklist

When producing a new downstream asset:

1. Start from the SVG in `brand/` — never recreate from memory.
2. Verify the output renders correctly at the target size (browser or `qlmanage -t -s 256 -o /tmp logo.svg`).
3. For fixed-size PNGs, render at 2 x target resolution and downsample if edge quality matters.
4. Embed or outline JetBrains Mono in any wordmark export that leaves the repo.

### Regenerating PNG exports

PNGs live in `brand/exports/`. To regenerate:

```bash
# macOS (what this repo used):
cd brand
sips -s format png -Z 512 logo.svg --out exports/logo-512.png
sips -s format png -Z 240 logo.svg --out exports/logo-240.png
sips -s format png -Z 128 logo.svg --out exports/logo-128.png
sips -s format png -Z 64  logo.svg --out exports/logo-64.png
sips -s format png -Z 32  logo.svg --out exports/logo-32.png
sips -s format png -Z 16  favicon.svg --out exports/logo-16.png

# Preferred (if installed):
brew install librsvg
rsvg-convert -w 512 logo.svg -o exports/logo-512.png
# (repeat for each size)
```

### Regenerating favicon.ico

The repo's `website/assets/favicon.ico` was produced from the old Q+arrow glyph and must be regenerated from the new `favicon.svg`. On macOS:

```bash
brew install imagemagick
cd /path/to/claude-picker
magick convert brand/exports/logo-16.png brand/exports/logo-32.png brand/exports/logo-64.png website/assets/favicon.ico
```

This is a **pending task** for the user / website-upgrade agent — `sips` cannot produce `.ico`.

---

## Custody

The source of truth for all brand assets is `/brand/` in the `claude-picker` repository. Any change to the mark or palette must land there as a pull request, not in a downstream fork, slide deck, or one-off social graphic.

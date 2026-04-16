# claude-picker Brand Guide

The official identity kit for **claude-picker**. If you are shipping something that represents the project — a website, a README header, a social profile, a package listing, a talk slide — use this guide.

---

## The mark

The primary mark is **"The Picker Caret"**: a filled right-pointing chevron next to a rounded pill. The chevron is the fzf cursor. The pill is the selected session row. Together they depict the one gesture the product performs: picking a session from a list.

Three traits make this mark work:

1. **One concept.** A caret pointing at a row. Every part of the mark means something.
2. **Monochromatic, geometric, stroke-free.** Every asset is a single fill color. No gradients, no glows, no outlines.
3. **Survives at 16×16.** The favicon is the chevron alone, the brand's shorthand.

---

## Color palette

| Role | Name | Hex | Notes |
|------|------|------|-------|
| Primary | Catppuccin Mauve | `#CBA6F7` | The only color the mark is ever drawn in, except the mono variant |
| Background (dark) | Catppuccin Base | `#1E1E2E` | Default backdrop for the mauve mark |
| Mono fill | Catppuccin Base | `#1E1E2E` | Used when the mark must be printed in a single dark ink on light |
| Text (dark bg) | Catppuccin Text | `#CDD6F4` | Body copy companion — not part of the mark |
| Text (light bg) | Catppuccin Base | `#1E1E2E` | Body copy companion on light |

No other colors. Do not introduce teals, reds, or brand accents. Part of the system's value is that one glance tells you it's from the Catppuccin-flavored CLI family.

---

## Typography

| Context | Font | Weight | Usage |
|---------|------|--------|-------|
| Wordmark | JetBrains Mono | Bold (700) | `claude-picker` lockup in headers, hero, social |
| Code blocks / CLI snippets | JetBrains Mono | Regular (400) | README code fences, website docs, terminal mockups |
| UI / prose | Inter or system-ui | Regular / Semibold | Website body copy only. Never used in the mark. |

JetBrains Mono is the brand's typographic signature because it is the native typeface of the terminal context the product lives in. Never substitute a proportional font in the wordmark — the monospaced rhythm is the point.

If a downstream consumer cannot embed JetBrains Mono (offline print, third-party embed), use the outlined `wordmark.svg` exported via Figma / Illustrator "Convert to outlines". Do not substitute a different mono.

---

## Sizes

| Asset | Minimum size | Typical use |
|-------|-------------|-------------|
| `logo.svg` (mark only) | 24 px wide | In-line buttons, mini badges |
| `logo.svg` | 64 px wide | GitHub avatar, npm card, PR header |
| `favicon.svg` | 16 px wide | Browser tab, iOS home-screen, share chiclet |
| `wordmark.svg` | 160 px wide | Nav header, footer, README hero |
| `logo-on-dark.svg` | 128 px wide | OG / social / dark-tile backgrounds |

Below the minimums the mark begins to lose fidelity. Do not scale below.

---

## Clearspace

Reserve clearspace equal to **1× the glyph's width** on all sides of the primary mark.

```
        ┌────────────────────────────┐
        │    X                       │
        │  ┌────┐                    │
   X    │  │ >- │   claude-picker    │    X
        │  └────┘                    │
        │    X                       │
        └────────────────────────────┘

        X = 1 × glyph width = 52u on the 52×40 grid
```

No other graphic element (photography, heading, rule) may enter this zone.

---

## Variants & where to use each

| File | Use |
|------|-----|
| `logo.svg` | Default mauve mark on any dark background. README header, website nav, Product Hunt post. |
| `logo-on-dark.svg` | Pre-composited tile. GitHub profile avatar, LinkedIn share, npm owner icon. |
| `logo-mono.svg` | Light backgrounds, stickers printed in a single dark ink, black-and-white docs. |
| `wordmark.svg` | Anywhere the brand name needs to be explicit: site header, footer, hero, launch announcement. |
| `favicon.svg` | Browser tab. Export to `favicon.ico` (multi-res 16/32/48) for legacy browsers. |

### Mapping onto the website

- **Nav bar (top-left)** → `wordmark.svg`, 180 px wide, 16 px clearspace.
- **Footer** → `wordmark.svg`, 140 px wide, 32 px above.
- **Hero pattern** → `logo.svg` at ~96 px as the first element above the H1.
- **Browser tab** → `favicon.svg`, linked as `<link rel="icon" type="image/svg+xml" href="/favicon.svg">`.
- **Open Graph image** → `logo-on-dark.svg` centered on a 1200×630 tile of `#1E1E2E`, with `claude-picker` wordmark below at 120 px. Export as PNG at 2×.

### Mapping onto GitHub

- **Repo avatar** → `logo-on-dark.svg` exported to 400×400 PNG.
- **README header** → `wordmark.svg`.
- **Social preview** → 1280×640 canvas, `logo-on-dark.svg` centered, tagline underneath in JetBrains Mono Regular.

### Mapping onto npm

- **Package README** → `wordmark.svg` hosted on the GitHub repo.
- **Owner avatar** → Same as GitHub avatar.

---

## Do

- Use the mauve mark on dark backgrounds.
- Use the mono mark on light or single-ink contexts.
- Keep 1× clearspace around the mark.
- Scale proportionally.
- Pair with JetBrains Mono.

## Don't

- Don't stretch, skew, or rotate the mark.
- Don't recolor the mark. (No pink, no blue, no rainbow, no gradient — ever.)
- Don't add drop shadows, glows, bevels, strokes, or 3D effects.
- Don't place the mark on a busy photographic background without a solid plate.
- Don't substitute a different monospace font for the wordmark.
- Don't create derivative marks that riff on Anthropic's Claude logo.
- Don't add an outline to the chevron.
- Don't invert just one half (e.g., dark chevron + light pill).
- Don't use the chevron-only favicon in contexts where the full mark fits.

---

## Export checklist

When producing a new downstream asset:

1. Start from the SVG in this folder — never recreate from memory.
2. Verify the output renders correctly at the target size using a browser and a raster render (e.g., `qlmanage -t -s 256 -o /tmp logo.svg`).
3. For fixed-size PNGs, render at 2× target resolution and downsample.
4. For favicons, export 16/32/48 PNGs and bundle into `favicon.ico` using `png2ico` or ImageMagick.
5. Embed or outline JetBrains Mono in any wordmark export that leaves the repo.

---

## Custody

The source of truth for all brand assets is `/brand/` in the `claude-picker` repository. Any change to the mark or palette must land there as a pull request, not in a downstream fork, slide deck, or one-off social graphic.

# Gemini Prompts for claude-picker Brand Assets

These prompts are provided as a **fallback and extension** pathway, not the primary source. The hand-written SVGs in this folder are authoritative. Use these prompts when:

- You need a **derivative asset** (OG image, social sticker, launch banner) that extends the identity.
- You need to **regenerate** a variant after the brand ever changes.
- You want a **raster mockup** of the mark on physical merchandise.

All prompts assume **Gemini 2.x (vision model)** and are tuned to yield a usable vector-like output. If you need true path-based SVG, pipe Gemini output through `vtracer` or open in Figma and retrace.

---

## Master style reference (include at the top of every prompt)

> **Style reference — claude-picker brand.**
> Color: single fill, hex `#CBA6F7` (soft mauve).
> Background: flat `#1E1E2E` unless stated otherwise.
> Geometry: strictly flat, geometric, filled shapes. No strokes, no gradients, no shadows, no glows, no 3D, no bevels, no lighting effects, no depth, no noise, no texture, no decoration.
> Composition: monochrome, center-weighted, generous margin, no extraneous elements.
> Reject: photography, illustration, drop shadows, outlines, letterforms not on grid, "AI slop" aesthetics, magnifying glasses, gears, hands, faces, mascots, starbursts.

---

## 1. `logo.svg` — primary mark

If you ever need to regenerate the mark, use this prompt. **Expected output: a PNG approximating the SVG.** Trace the output in Figma to get a usable vector.

```
[Master style reference here]

Design a minimal vector-style logo mark on a transparent background.

Composition: two shapes side by side, horizontally centered.
- LEFT SHAPE: a bold, filled right-pointing chevron (the mathematical "greater than" symbol). Thick uniform body, 3:4 aspect ratio, pointed apex aimed right. Color #CBA6F7.
- RIGHT SHAPE: a rounded pill (horizontal stadium shape). Width roughly equal to the chevron's width. Height roughly a quarter of the chevron's height. Vertically centered on the chevron's apex. Color #CBA6F7.
- GAP: small deliberate negative space between chevron tip and pill, roughly 10% of the chevron's height.

Both shapes same mauve color, flat, no texture, no stroke.
Output: square canvas, transparent background, 1024×1024.
```

---

## 2. `logo-on-dark.svg` as a social tile

```
[Master style reference here]

Render a 1200×630 Open Graph social card.
- Background: solid #1E1E2E, edge-to-edge.
- Centered horizontally and vertically at 40% of the canvas height: the claude-picker mark — a mauve filled chevron ">" next to a rounded mauve pill. Mark height ~30% of canvas height.
- Below the mark (at 70% canvas height): the wordmark "claude-picker" in JetBrains Mono Bold, color #CBA6F7, letter-spacing tight.
- Below the wordmark (at 82% canvas height): tagline "Terminal session manager for Claude Code." in JetBrains Mono Regular, color #CDD6F4, 24pt.
- No other elements. No dots, no URLs, no taglines beyond the one specified.
- Strictly no gradients, no shadows, no glows.
```

---

## 3. GitHub repo avatar (square 400×400)

```
[Master style reference here]

Render a 400×400 square tile.
- Background: solid #1E1E2E, full-bleed.
- Centered: the claude-picker mark — a filled chevron ">" next to a rounded pill, both in #CBA6F7.
- Mark occupies roughly 50% of the tile's width, horizontally and vertically centered with equal margins.
- No wordmark. No tagline. No text. No border. No noise.
```

---

## 4. Favicon (pixel-perfect 32×32)

```
[Master style reference here]

Render a 32×32 pixel icon, pixel-snapped.
- Background: transparent.
- Centered: a single filled chevron ">" in #CBA6F7, occupying ~24×24 of the canvas.
- No pill, no text, no other shapes.
- Crisp edges, no antialiasing softness beyond what is necessary.
```

---

## 5. Launch / Product Hunt sticker (square, 1024×1024)

```
[Master style reference here]

Design a square launch sticker.
- Background: solid #1E1E2E.
- Top 60%: centered claude-picker mark in mauve #CBA6F7 (filled chevron ">" + rounded pill), mark height ~40% of canvas.
- Bottom 35%: wordmark "claude-picker" in JetBrains Mono Bold #CBA6F7, followed below by a small inline label "v1.0 • Launching Now" in JetBrains Mono Regular #CDD6F4.
- Negative space between wordmark and edge: 10% of canvas on all sides.
- No confetti, no burst shapes, no rocket emoji, no additional graphics.
```

---

## 6. Terminal hero illustration (for website hero section)

```
[Master style reference here]

Design a wide terminal-mockup hero illustration, 1440×720.
- Background: solid #1E1E2E.
- Centered: a stylized terminal window shape — rounded rectangle, 1100×500, outlined with a 4px #CBA6F7 stroke ONLY on its rounded-rectangle perimeter (the only stroke permitted in this single asset). Fill: #11111B (slightly darker than background).
- Inside the terminal: three horizontal "rows" drawn as filled pills of #CBA6F7, each 60px tall, stacked vertically with 20px gaps. First row fully saturated (selected); other two rows at 30% opacity (unselected).
- To the left of the top (selected) row, inside the terminal frame: a filled mauve chevron ">" matching the logo, 48px tall.
- No icons, no text, no other decoration. The chevron + selected row visually echoes the main brand mark.
```

---

## 7. Merchandise — sticker die-cut template

```
[Master style reference here]

Design a die-cut vinyl sticker, 3 inches wide.
- Overall shape: the outline of the claude-picker mark — chevron plus pill — kissed-cut with a 3mm white bleed border.
- Fill: #CBA6F7 solid.
- Background (visible beyond the cut): transparent.
- No extra decoration.
```

---

## Prompt-engineering notes

- Gemini will occasionally add a drop shadow or subtle gradient unless you say "strictly flat, no gradients, no shadows" — always include that phrase.
- If the chevron comes out too thin, add "thick uniform stroke body, not a wireframe".
- If the pill comes out as a rectangle, say "rounded stadium shape with fully rounded semicircular ends".
- Always review Gemini output against the hand-drawn SVGs in this folder — the SVG is the source of truth, the generated image is always a proposal.

---

## When NOT to use Gemini

- Do not use Gemini to regenerate `logo.svg`, `favicon.svg`, `logo-on-dark.svg`, `logo-mono.svg`, or `wordmark.svg`. Those are hand-authored and live in-repo as the authoritative source.
- Do not use Gemini output as a final production asset without first tracing to clean SVG.
- Do not use Gemini for any asset that will be used in a legal or trademark context (Product Hunt logo field, npm package icon, etc.) — those must come from the hand-authored SVGs.

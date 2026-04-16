# claude-picker — Logo Concepts & Final Pick

> Research, three concepts, the honest story of how we got here, and why the 4-dot mark won.

---

## Research: How CLI / dev tools brand themselves

I studied the leading marks in this space to understand the shared visual language:

| Tool | Mark | Lesson |
|------|------|--------|
| **fzf** | No real logo — just a text wordmark `fzf` | CLI tools can live on typography alone; the loop is the brand |
| **Raycast** | Red/pink "R" with a beam descender | Monogram + single metaphorical flourish |
| **Linear** | Three stacked lines inside a rounded square | One geometry, one color, one idea |
| **Warp** | Stylized "W" with a warping tail | Monogram that encodes the name's meaning |
| **Charm.sh tools** (VHS, Gum, Freeze, Skate, Soft Serve) | Each has a single flat icon; gum = bubble, freeze = snowflake, skate = board | A noun-glyph per tool, all in the same pastel palette |
| **Starship** | Rocket chevron `>_` riff | The prompt symbol is the logo |
| **bat** | Bat silhouette (emoji-adjacent) | Name puns can work if the shape is clean |
| **lazygit / lazydocker** | Text wordmark, no mark | Tiny projects often skip the mark entirely |
| **Notion** | Bracketed block grid `[::]` | A UI primitive can itself be the logo — the product is "blocks", the mark is a block |
| **Arc Browser** | Minimal square + asymmetric dot | Abstract spatial composition, single color |

### Patterns I noted

1. **Monochromatic.** 9 of 10 are one color on one bg. No gradients.
2. **Geometric.** Straight lines, simple curves, primitive shapes.
3. **One idea.** Every successful mark encodes exactly one concept.
4. **Prompt-native OR UI-primitive.** Tools borrow either the shell's alphabet (`>`, `_`, `$`) OR the product's own UI primitive (Notion's block, Arc's canvas square).
5. **No trademarked borrowing.** None of them riff on a parent-company mark (no fake OpenAI, no fake Anthropic). Doing so looks cheap and is legally shaky.

---

## Design brief recap

- **Product essence:** A fuzzy picker over your Claude Code session history. You hit `Ctrl+P`, a list appears, you preview and resume.
- **Brand associations:** terminal, picking/selecting, sessions, fast, native, Unix.
- **Color:** Catppuccin Mauve `#CBA6F7` on base `#1E1E2E`.
- **Must survive at 16 x 16** (favicon).

---

## Concept 1 — "Q + arrow" (retired)

A stylised `Q` with an arrow descender, suggesting "query → result".

### Why it was tried
Matched the hand-drawn feel of early CLI logos. Felt friendly.

### Why it's retired
- The `Q` doesn't encode the product. It's just a letter. The arrow is a cliche.
- At 16 x 16 the arrow collapses into a messy tail.
- It looks like ten other "AI search" marks. Nothing about it says "session picker over your Claude Code history".
- Most importantly: **the AI-generated LinkedIn / Product Hunt / Instagram slides don't use it**. Gemini produced them with the 4-dot mark. If we kept Q+arrow we'd have to regenerate every social asset.

**Verdict:** Retired. Generic, doesn't encode purpose, and creates brand drift vs. the social material already in circulation.

---

## Concept 2 — "The Picker Caret" (chevron + pill)

A filled right-pointing chevron next to a rounded pill — the fzf cursor landing on a selected row.

```
 >  ▄▄▄▄▄▄▄▄▄▄▄
 >  █          █
 >  ▀▀▀▀▀▀▀▀▀▀▀
```

### Why it was designed first
- `>` is the Unix shell's native selection glyph.
- Two primitives, one line: cursor + selected row. Literal translation of the product gesture.
- Scales cleanly. The chevron survives at 16 x 16.
- Distinctive — no CLI tool owns "caret + row" silhouette.

This concept was built out first and shipped as the original `logo.svg`, `logo-on-dark.svg`, `favicon.svg`, and wordmark lockup.

### Why it's now secondary
While building the social material, Gemini produced LinkedIn carousel slides, a Product Hunt thumbnail, and Instagram stories using a **4-dot grid** in place of a generic "logo glyph" placeholder. The slides were visually excellent and already public in our materials pipeline. We had two choices:

1. Force the chevron mark back in, regenerate every slide, lose the gen-AI quality we liked.
2. Adopt the 4-dot as canonical and retire the chevron to a secondary role.

We chose option 2 because:

- **Brand equity was already forming around the 4-dot mark** through the social carousel. Users who saw the LinkedIn post and then land on the site should see the same mark.
- **The 4-dot encodes the product literally.** "A picker showing 4 items, with the cursor on one." You can't get more on-brief.
- **The chevron still has a lane** — merch, interior branding, CLI-native surfaces. Keeping it as `logo-alt-chevron.svg` preserves the work without creating identity confusion.

**Verdict:** Demoted to alternate / secondary mark. See `brand.md` § Alternate mark.

---

## Concept 3 — RECOMMENDED: "The 4-Dot Picker"

A 2 x 2 grid of four dots: three rounded squares + one circle (bottom-right).

```
    ▢ ▢
    ▢ ●
```

### The reading

Every element has a meaning:

- Each of the four dots = a session row in the picker.
- The three rounded squares = unselected sessions.
- The one circle in the bottom-right = the currently selected session. The cursor.

The **shape difference** between square and circle does the work a highlight color would do in a real UI. The logo stays monochromatic and still communicates "one is selected".

### Geometry (viewBox 64 x 64)

| Element | Position | Size |
|---------|----------|------|
| Top-left square | (6, 6) | 24 x 24, r=6 |
| Top-right square | (34, 6) | 24 x 24, r=6 |
| Bottom-left square | (6, 34) | 24 x 24, r=6 |
| Bottom-right circle | center (46, 46) | r=10 |
| Gap between dots | 4u | |
| Edge clearspace | 6u | |

The circle is 20u in diameter vs. 24u for the squares — deliberately smaller so the difference reads. If they were the same size, the circle would feel like a rotated square. At 20/24 ratio, the circle reads as a distinct shape.

### Why this wins

- **Matches the product literally.** The logo is a freeze-frame of the picker view.
- **Brand continuity.** Same silhouette as the AI-generated LinkedIn / PH / IG assets already published.
- **Monochrome-friendly.** Four shapes, one fill, no strokes, no gradients.
- **Survives at 16 x 16.** All four dots remain distinguishable (tested by rendering `favicon.svg` at native 16px — the circle is 5px diameter, the squares are 6 x 6px with 1.5px corners; the contrast is clear).
- **Distinct in the category.** No dev-tool mark I surveyed uses a 2 x 2 UI-grid. Notion's brackets come closest but read as brackets, not dots.
- **Scales up beautifully.** At 512px the rounded corners (6u = ~48px at 512-scale) feel soft without being cartoonish.

### Minimum-size behavior

At 16 x 16 the dots are close to touching but remain distinct. The favicon variant tightens internal spacing (2px gaps, 1-unit outer margin) so the grid uses every pixel. No special simplification — same concept, same count.

---

## Final decision

**Concept 3 — The 4-Dot Picker.** Shipped as:

- `logo.svg` — primary mauve mark (64 x 64 viewBox)
- `logo-on-dark.svg` — 128 x 128 dark tile
- `logo-mono.svg` — dark mono on light
- `wordmark.svg` — glyph + `claude-picker` in JetBrains Mono Bold
- `favicon.svg` — pixel-aligned 16 x 16 viewBox variant
- `logo-alt-chevron.svg` — preserved secondary mark

PNGs at 16 / 32 / 64 / 128 / 240 / 256 / 512 px in `brand/exports/`.

Q + arrow is retired. The Picker Caret chevron is kept as a secondary motif with a defined lane and is not interchangeable with the primary mark.

# claude-picker — Logo Concepts & Recommendation

> Research, three concepts, and the final pick. All marks are monochrome, geometric, and built to survive at 16×16.

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

### Patterns I noted

1. **Monochromatic.** 9 of 10 are one color on one bg. No gradients.
2. **Geometric.** Straight lines, simple curves, primitive shapes.
3. **One idea.** Every successful mark encodes exactly one concept.
4. **Prompt-native.** Starship and many terminal tools borrow `>`, `_`, `$`, `❯` — the shell's own alphabet is free visual vocabulary.
5. **No trademarked borrowing.** None of them riff on a parent-company mark (no fake OpenAI, no fake Anthropic). Doing so looks cheap and is legally shaky.

---

## Design brief recap

- **Product essence:** A fuzzy picker over your Claude Code session history. You hit `Ctrl+P`, a list appears, you preview and resume.
- **Brand associations:** terminal, picking/selecting, sessions, fast, native, Unix.
- **Color:** Catppuccin Mauve `#CBA6F7` on base `#1E1E2E`.
- **Must survive at 16×16** (favicon).

---

## Concept A — "cp" monogram (dropped)

```
 ╭───╮
 │ c │ p
 ╰───╯
```

Two letters, the `c` as a 270° open arc, the `p` as a filled descender. Clean enough, but:
- Every tool with a two-word name uses a monogram.
- Doesn't say anything about what the product DOES.
- Risks looking like "copy-paste" because `cp` is already a Unix command.

**Verdict:** Rejected. The existing Unix `cp` meaning is a liability, not an asset.

---

## Concept B — "picker bracket + cursor" (dropped)

A magnifying-glass circle fused with a `>` chevron handle.

```svg
<!-- sketch -->
<circle cx="24" cy="24" r="14" fill="none" stroke="#CBA6F7" stroke-width="4"/>
<path d="M34 34 L44 44" stroke="#CBA6F7" stroke-width="4" stroke-linecap="round"/>
<path d="M18 18 L26 24 L18 30" stroke="#CBA6F7" stroke-width="4" fill="none"/>
```

Tells the "find/pick" story. But:
- Needs strokes to read, making it fussy at 16px.
- Magnifying glasses are the most overused dev-tool icon.
- Too close to Spotlight, Raycast, every search bar ever.

**Verdict:** Rejected. Not distinctive enough.

---

## Concept C — "stacked conversation" (dropped)

Three horizontal pills of varying widths, stacked with offsets — a session list.

```
 ▮▮▮▮▮▮▮▮▮▮▮▮▮▮   ← selected (full width, filled)
  ▮▮▮▮▮▮▮▮▮▮       ← unselected
  ▮▮▮▮▮▮▮▮▮▮▮▮     ← unselected
```

Great metaphor for the UI. But three pills at 16px collapse into a blurry block. And the mark reads as "text lines" more than "picker", pushing it toward generic "document" iconography.

**Verdict:** Rejected on the 16px test.

---

## Concept D — Claude monogram riff

Explicitly listed in the brief as risky. Confirmed: no.

---

## Concept E — RECOMMENDED: "The Picker Caret"

A single composite glyph that reads simultaneously as:

1. The **`>` chevron** — every Unix shell prompt, every fzf row cursor.
2. The **horizontal line** to its right — the "selected session" row in fzf.
3. A **caret selecting a line** — the exact gesture of the product.

### Construction (on a 64×64 grid)

```
     ┌─────────────────────────────┐
     │                             │
     │    ▲                        │
     │   ╱ ╲                       │
     │  ╱   ╲      ▄▄▄▄▄▄▄▄▄▄▄     │
     │ ╱     ╲     █          █    │
     │╱       ╲    ▀▀▀▀▀▀▀▀▀▀▀     │
     │╲       ╱                    │
     │ ╲     ╱                     │
     │  ╲   ╱                      │
     │   ╲ ╱                       │
     │    ▼                        │
     │                             │
     └─────────────────────────────┘
```

- Left: a solid filled chevron `>` pointing right (the picker cursor).
- Right: a horizontal pill (the selected session line).
- Negative space between them is the visual "click point" — where the user's attention lives.

### Geometry

- Chevron: isoceles triangle pointing right, 20×28 units, positioned top-left with stroke thickness `~6u` expressed as a filled polygon (no stroke — all fills).
- Pill: rounded rectangle, width 28u, height 8u, radius 4u, vertically centered on the chevron's point.
- Gap between chevron tip and pill: 6u — a deliberate negative-space column.

### Why this wins

- **One shape.** A chevron + a bar. Two primitives, one idea.
- **Semantically loaded.** Every developer reads `>` as "prompt / select / enter".
- **Scales.** At 16px, the chevron survives and the pill remains a horizontal bar. The favicon variant just thickens the pill.
- **Distinctive.** I looked — no major CLI tool owns this exact mark. Starship uses `>_` but with an underscore. Raycast uses an R. We'd own the "caret + row" silhouette.
- **Matches product.** When a user hits `Ctrl+P` and sees fzf's `>` cursor move down a session list, the logo is a literal freeze-frame of that moment.
- **Monochrome-friendly.** Pure fills, single color, no strokes, no gradients. Renders identically in every context.

### Favicon simplification

At 16×16, drop internal padding and thicken both shapes by ~20%. The chevron becomes a solid right-pointing triangle; the pill becomes a 2-unit-thick bar. Still one shape, still one idea.

---

## Final decision

**Concept E — The Picker Caret.** Proceeding to build `logo.svg`, `logo-on-dark.svg`, `logo-mono.svg`, `wordmark.svg`, and `favicon.svg`.

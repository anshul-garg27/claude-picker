# Image Generation Prompts — Complete Edition

Production-ready image briefs for `claude-picker` — the terminal session manager for Claude Code. Each prompt is copy-pasteable, tool-agnostic, and engineered to produce a usable asset on the first generation. Primary target: **Gemini AI Pro**. Fallbacks: **OpenAI DALL-E 3 / GPT-Image**, **Midjourney v6**, **Flux.1 Pro**.

---

## Design System

### Catppuccin Mocha Palette (strict)

| Token | Hex | Usage |
|---|---|---|
| Base (bg) | `#1E1E2E` | Default canvas |
| Mantle | `#181825` | Terminal window fill |
| Crust | `#11111B` | Deepest recesses, editorial backdrops |
| Surface 0 | `#313244` | Inline terminal bg, pill fills |
| Surface 1 | `#45475A` | Borders, dividers |
| Surface 2 | `#585B70` | Heavier dividers |
| Overlay 0 | `#6C7086` | Muted text, timestamps |
| Overlay 1 | `#7F849C` | Secondary muted text |
| Text | `#CDD6F4` | Primary foreground |
| Subtext 1 | `#BAC2DE` | Secondary foreground |
| Subtext 0 | `#A6ADC8` | Tertiary foreground |
| Lavender | `#B4BEFE` | Secondary accent |
| Blue | `#89B4FA` | User messages, links |
| Sapphire | `#74C7EC` | Info |
| Sky | `#89DCEB` | Info-light |
| Teal | `#94E2D5` | Secondary info |
| Green | `#A6E3A1` | Session names, success, cost values |
| Yellow | `#F9E2AF` | AI messages, emphasis, bullets |
| Peach | `#FAB387` | Warnings, secondary headers |
| Maroon | `#EBA0AC` | Soft alerts |
| Red | `#F38BA8` | Errors, destructive |
| Mauve | `#CBA6F7` | **Brand primary**, pointers, glows |
| Pink | `#F5C2E7` | Highlights |
| Flamingo | `#F2CDCD` | Soft highlight |
| Rosewater | `#F5E0DC` | Softest highlight |

### Typography (default stack, all images)

- **Monospace (terminals, code):** JetBrains Mono → Fira Code → SF Mono → Menlo. Weight 400 regular, 700 bold. Rendered at 16–22 px in mockups.
- **Sans headings (editorial):** Inter → Söhne → General Sans → system-ui. Weight 600–700 for titles, 400–500 for body.
- **Letter-spacing:** -0.01em on large display text, 0 on body, +0.04em on small caps labels.

### Mood Keywords (apply to every image)

`moody · editorial · precise · dark · premium · understated · developer-grade`

### Global Do's

- Use the exact hex values above; no approximations.
- Terminals have rounded corners (radius 8–12 px) and three traffic-light dots in the top-left (red `#F38BA8`, yellow `#F9E2AF`, green `#A6E3A1`) at ~8 px diameter, or a flat flush bar with no chrome — pick one per image and be consistent.
- Monospace columns must truly align vertically (the generator must respect column grid).
- Glows are allowed only where specified, always as a single large-radius halo at 10–15% opacity of a palette hue — never a neon outline.

### Global Don'ts (universal negative prompt)

`No emojis. No stock photography. No human figures. No 3D bevels or skeuomorphism. No plasticky glass reflections. No neon/cyberpunk glow. No gradients unless explicitly specified. No drop shadows on text. No lens flares. No bokeh. No watermarks. No logo pileups. No garbled/lorem-ipsum text. No UUIDs longer than 8 characters visible. No mismatched fonts within one image. No color outside the Catppuccin Mocha palette.`

---

## Image Catalog (one-line summary)

| # | Title | Dimensions | Primary use |
|---|---|---|---|
| 1 | GitHub Social Preview | 1280x640 | Repo social preview, top-of-README |
| 2 | Medium Hero Image | 1400x788 | Medium article top, long-form hero |
| 3 | Before vs After Comparison | 1200x600 | Medium inline (the problem → solution) |
| 4 | Architecture Diagram | 1200x800 | README architecture section, Medium inline |
| 5 | Open Graph / LinkedIn Share | 1200x630 | Link unfurl across LinkedIn, Slack, Discord |
| 6 | Project Picker Screenshot | 1200x600 | Docs/Medium step-1 illustration |
| 7 | Session Picker + Preview Panel | 1200x600 | Docs/Medium step-2 illustration |
| 8 | `--search` Full-Text Search | 1200x600 | Feature spotlight (search) |
| 9 | `--stats` Dashboard | 1200x800 | Feature spotlight (analytics) |
| 10 | `--tree` Session Tree | 1200x800 | Feature spotlight (forks) |
| 11 | `--diff` Side-by-Side | 1400x800 | Feature spotlight (comparison) |
| 12 | Bookmarks (Ctrl+B) | 1200x630 | Feature spotlight (pinning) |
| 13 | Export to Markdown (Ctrl+E) | 1200x630 | Feature spotlight (export) |
| 14 | Cost Tracking Highlight | 1200x675 | Feature spotlight (cost/token) |
| 15 | Logo / Icon | 512x512 | GitHub avatar, npm avatar, social profile |
| 16 | Favicon Variant | 64x64 | Docs site favicon, tab icon |
| 17 | Twitter Card (generic) | 1200x675 | Any tweet without GIF |
| 18 | Reddit Thumbnail | 1200x630 | r/commandline, r/ClaudeAI, r/programming posts |
| 19 | Product Hunt Gallery | 1270x760 | PH listing gallery slots 2–5 |
| 20 | Product Hunt Thumbnail | 240x240 | PH tile, newsletter, thumbnail feed |
| 21 | Instagram Story Template | 1080x1920 | Any IG/Threads story slide |
| 22 | LinkedIn Carousel Template | 1080x1350 | Any LinkedIn carousel slide |
| 23 | Claude Code Skill Card | 1200x630 | `/claude-picker` skill launch post |
| 24 | Warp Integration Card | 1200x630 | Warp blocks integration announcement |
| 25 | Age-Warning Color Key | 1200x600 | Docs inline reference (color-coded timestamps) |

---

## 1. GitHub Social Preview — 1280x640 — GitHub repo social preview, README top banner

**Where it's used:** GitHub `Settings → Social Preview`, auto-unfurled when the repo URL is shared in Slack/Discord/Twitter. This is the single highest-impact asset.

**Safe zone:** Keep all critical text and the terminal mockup within a central **960x420** area (160 px horizontal margin, 110 px vertical margin). GitHub cards crop ~5% on narrow viewports.

**Background:** Flat `#1E1E2E` (Base). No gradient. A single extremely subtle vignette is acceptable — a radial darkening toward the corners down to `#181825` at the extreme edges, opacity max 20%.

**Layout:** Split 50/50 vertical. Left column = wordmark + tagline. Right column = terminal mockup.

**Prompt (primary — Gemini AI Pro / DALL-E 3 / Flux.1 Pro):**

```
A wide 1280x640 developer marketing image, flat dark Catppuccin Mocha background #1E1E2E, split into two vertical halves.

LEFT HALF (x: 80-580, vertically centered):
Large monospace wordmark "claude-picker" in 72px JetBrains Mono Bold, color #CDD6F4, letter-spacing -0.01em.
Below it, 22px gap.
Tagline in 22px Inter Medium, color #A6ADC8: "find, preview, and resume your Claude Code sessions"
Below the tagline, 28px gap, three pill badges in a row, each 28px tall, 14px horizontal padding, 8px radius, fill #313244, 1px border #45475A, text 13px JetBrains Mono in #BAC2DE:
  "bash + python + fzf"  "432 lines"  "v1.2"

RIGHT HALF (x: 680-1200, vertically centered):
A terminal window mockup, 500x400 px, rounded corners radius 10px, fill #181825, 1px border #45475A.
Top chrome bar 28px tall, fill #1E1E2E, with three 10px circles left-aligned: #F38BA8, #F9E2AF, #A6E3A1, each 10px apart. Tab label centered: "architex — claude-picker" in 12px JetBrains Mono, color #6C7086.

Inside the terminal, monospace 16px, 24px line-height, 20px left padding:
  Line 1 (header): "claude-picker" in #CBA6F7 bold + two spaces + "architex" in #89B4FA
  Line 2 (blank)
  Line 3: "── named ──" in #6C7086
  Line 4: "▸ " in #CBA6F7 + "●" in #F9E2AF + " auth-refactor" in #A6E3A1 bold + right-aligned "5m · 45 msgs" in #6C7086
  Line 5: "  ●" in #F9E2AF + " fix-race-condition" in #A6E3A1 + right-aligned "2h · 28 msgs" in #6C7086
  Line 6: "  ●" in #F9E2AF + " drizzle-migration" in #A6E3A1 + right-aligned "1d · 67 msgs" in #6C7086
  Line 7 (blank)
  Line 8: "── recent ──" in #6C7086
  Line 9: "  ○" in #6C7086 + " session" in #6C7086 + right-aligned "4h · 12 msgs" in #6C7086
  Line 10 (blank)
  Line 11: "session >" in #89DCEB

Subtle glow around the terminal window: single radial halo of #CBA6F7 at 12% opacity, 80px blur radius, no hard edge.

Mood: moody, editorial, precise, dark, premium. Inspired by Linear, Raycast, and Warp marketing pages.

NEGATIVE: no emoji, no gradients on background, no 3D, no skeuomorphism, no human figures, no stock photos, no neon, no lens flare, no lorem ipsum, no logo pile, no UUIDs, no garbled monospace columns, no decorative icons except the three traffic-light circles.
```

**Alternate (Midjourney v6):**

```
Wide developer tool hero, 1280x640, dark Catppuccin Mocha #1E1E2E base, left side bold monospace wordmark "claude-picker" in #CDD6F4, right side clean terminal window mockup on #181825 listing colored session names in green and mauve, subtle mauve halo glow behind the terminal, editorial, minimal, Linear Raycast aesthetic, flat design, no emoji, no gradients, no 3D --ar 2:1 --style raw --s 50 --v 6
```

---

## 2. Medium Hero Image — 1400x788 — Top of Medium article, Substack header

**Where it's used:** Hero image for the Medium / Substack / personal blog long-form post. First thing readers see in their feed.

**Safe zone:** Central **1120x630** for all content. Medium aggressively crops the top and bottom ~8% in the feed preview.

**Background:** Deep `#11111B` (Crust). Add an extremely subtle grid: 1px lines every 80px at `#CDD6F4` 3% opacity, fading toward the edges via a radial mask so the center is clearer.

**Prompt:**

```
Cinematic 1400x788 editorial hero image for a developer article. Deep black background #11111B. Subtle grid pattern: 1px lines every 80px at 3% white opacity, fading radially toward the center so the periphery feels darker.

CENTER STAGE (x: 260-1140, y: 200-580):
A terminal window 880x380 px, rounded corners 12px, fill #1E1E2E, 1px border #45475A. Chrome bar 30px tall with three circles (#F38BA8, #F9E2AF, #A6E3A1). Tab label: "architex ▸ auth-refactor" in 12px JetBrains Mono #6C7086.

Inside, split 60/40 vertical. Thin vertical divider #45475A.

LEFT 60% (session list, 20px padding, 16px mono, 24px line-height):
  "── named ──" in #6C7086
  "▸ ● auth-refactor     5m   45 msgs" (pointer #CBA6F7, dot #F9E2AF, name #A6E3A1, timestamp #6C7086)
  "  ● payment-gateway   2h   28 msgs" (all colors as above)
  "  ● k8s-deployment    1d   67 msgs"
  ""
  "── recent ──" in #6C7086
  "  ○ session           4h   12 msgs" (dot #6C7086, name #6C7086)

RIGHT 40% (preview panel, 16px padding, 15px mono):
  "auth-refactor" in #A6E3A1 bold
  "2026-04-14 · 45 msgs" in #6C7086
  "── · ──" in #45475A
  "you" in #89B4FA bold, then " the auth middleware is storing session tokens in plain cookies..." in #CDD6F4, wrapped naturally
  "ai" in #F9E2AF bold, then " I'll restructure the token storage to use encrypted httpOnly cookies with..." in #CDD6F4
  "you" in #89B4FA bold, then " also need refresh token rotation" in #CDD6F4

Terminal carries a soft mauve halo: #CBA6F7 at 14% opacity, 140px blur, no hard edge, no second glow.

ABOVE THE TERMINAL (x: 260-1140, y: 90-180), centered:
Title in 44px Inter SemiBold, color #CDD6F4, letter-spacing -0.01em:
  "I Reverse-Engineered How Claude Code Stores Sessions"

BELOW THE TERMINAL (y: 620-690), centered:
Subtitle in 20px Inter Regular, color #FAB387:
  "432 lines of bash + python + fzf"

BOTTOM META STRIP (y: 710-740), centered:
16px Inter Regular, color #6C7086:
  "by Anshul Garg   ·   12 min read   ·   April 2026"

Mood: atmospheric, editorial, moody, premium, Wired-magazine-for-developers.

NEGATIVE: no emoji, no stock photo, no human figures, no 3D, no skeuomorphism, no cyberpunk neon, no bokeh, no gradients beyond the single halo, no drop shadows on text, no lens flare, no garbled text.
```

---

## 3. Before vs After Comparison — 1200x600 — Medium inline, Twitter reply card

**Where it's used:** The "problem → solution" moment in the Medium article. Also works as a standalone Twitter reply image.

**Safe zone:** Each panel has a central **540x500** content zone with 30 px padding.

**Background:** Solid `#11111B` (Crust) across the whole image. A single 2px vertical divider at x=600, color `#45475A`.

**Prompt:**

```
Wide 1200x600 side-by-side comparison on a flat #11111B background. A single 2px vertical divider at x=600, color #45475A.

LEFT PANEL (x: 0-600):
Header at y=50, centered: "BEFORE" in 18px Inter Bold, letter-spacing 0.12em, color #F38BA8.
Sub-caption at y=82, centered: "plain claude --resume" in 14px Inter Regular, color #6C7086.

Terminal mockup at y=120, 540x380, rounded 10px corners, fill #1E1E2E, 1px border #45475A, chrome bar with dots muted to #45475A (dimmed intentionally).

Terminal content in 15px JetBrains Mono, all color #7F849C (muted, low contrast — deliberately unreadable feeling):
  "? Pick a conversation to resume"
  ""
  "  4a2e8f1c-9b3d-4e7a-a8b2-f1c9e3d5...  (2 hours ago)"
  "  b7c9d2e0-1f4a-8b6c-d0e7-3a8b7c9d...  (3 hours ago)"
  "  e5f8a3b1-7c2d-9e0f-1a2b-8c4d2e1f...  (yesterday)"
  "  2d6e4f8a-c1b3-5d7e-9f0a-b2c6d4e1...  (yesterday)"
  "  1a7b9c4d-5e8f-2a6b-3c1d-7e9f0a2b...  (2 days ago)"
  ""
  "(use arrow keys)"

A tiny red badge bottom-right of the terminal: "wtf is this" in 12px Inter Medium, color #F38BA8, semi-transparent.

RIGHT PANEL (x: 600-1200):
Header at y=50, centered: "AFTER" in 18px Inter Bold, letter-spacing 0.12em, color #A6E3A1.
Sub-caption at y=82, centered: "claude-picker" in 14px Inter Regular, color #6C7086.

Terminal mockup at y=120, 540x380, rounded 10px, fill #181825, 1px border #45475A, chrome bar with full-color dots (#F38BA8, #F9E2AF, #A6E3A1).

Terminal content in 15px JetBrains Mono, 22px line-height:
  "claude-picker" in #CBA6F7 bold + "   architex" in #89B4FA
  ""
  "── named ──" in #6C7086
  "▸ ● auth-refactor         5m · 45 msgs" (pointer #CBA6F7, dot #F9E2AF, name #A6E3A1, meta #6C7086)
  "  ● fix-race-condition    2h · 28 msgs"
  "  ● drizzle-migration     1d · 67 msgs"
  ""
  "── recent ──" in #6C7086
  "  ○ session               4h · 12 msgs" (all #6C7086)
  ""
  "session >" in #89DCEB

Mood: before = frustrating, cluttered, visually flat. After = clean, confident, organized.

NEGATIVE: no emoji, no gradient backgrounds, no decorative icons, no human reactions, no stock photo, no 3D, no neon. Both terminals must show the same font at the same size — only color treatment differs.
```

---

## 4. Architecture Diagram — 1200x800 — Medium inline, README "How it works"

**Where it's used:** Medium article mid-body diagram, README `## Architecture` section.

**Safe zone:** Central **1000x680** with 100 px horizontal and 60 px vertical margins.

**Background:** Flat `#1E1E2E`. No grid. No vignette.

**Prompt:**

```
1200x800 clean architecture diagram on flat #1E1E2E background. No grid, no vignette.

Seven rounded-corner boxes arranged in a flow, connected by thin 1.5px arrow lines color #6C7086 with small 6px arrowheads.

BOX 1 (top-left, x: 120-420, y: 80-180):
Fill #181825, 1px border #89B4FA, 8px radius.
Inside: "~/.claude/projects/" in 18px JetBrains Mono Bold #CDD6F4, centered.
Below: "JSONL session files" in 13px Inter Regular #6C7086, centered.

BOX 2 (top-right, x: 780-1080, y: 80-180):
Fill #181825, 1px border #89B4FA, 8px radius.
Inside: "~/.claude/sessions/" in 18px JetBrains Mono Bold #CDD6F4.
Below: "metadata (name, bookmarks)" in 13px Inter Regular #6C7086.

BOX 3 (middle-center, x: 420-780, y: 280-420):
Fill #181825, 2px border #CBA6F7, 10px radius.
Subtle mauve halo around the box: #CBA6F7 at 15% opacity, 40px blur.
Inside (stacked):
  "claude-picker" in 28px JetBrains Mono Bold #CBA6F7, centered.
  "entry point" in 14px Inter Medium #BAC2DE, centered.
  A horizontal line in #45475A.
  "~/.local/bin/claude-picker" in 12px JetBrains Mono #6C7086.

BOX 4 (bottom-left, x: 80-380, y: 520-640):
Fill #181825, 1px border #A6E3A1, 8px radius.
Inside: "session-list.sh" in 16px JetBrains Mono Bold #CDD6F4.
Below: "builds fzf list, handles sort" in 12px Inter Regular #6C7086.

BOX 5 (bottom-right, x: 820-1120, y: 520-640):
Fill #181825, 1px border #A6E3A1, 8px radius.
Inside: "session-preview.py" in 16px JetBrains Mono Bold #CDD6F4.
Below: "renders Rich-formatted preview" in 12px Inter Regular #6C7086.

BOX 6 (bottom-center, x: 450-750, y: 680-760):
Fill #181825, 1px border #F9E2AF, 8px radius.
Inside: "fzf 0.58+" in 16px JetBrains Mono Bold #CDD6F4.
Below: "interactive picker + borders" in 12px Inter Regular #6C7086.

BOX 7 (far right of BOX 6, x: 880-1120, y: 680-760):
Fill #181825, 1px border #FAB387, 8px radius.
Inside: "claude --resume" in 16px JetBrains Mono Bold #CDD6F4.
Below: "opens session" in 12px Inter Regular #6C7086.

ARROWS (thin 1.5px, #6C7086, gentle orthogonal routing, 6px arrowheads):
  BOX 1 → BOX 3 (down-right bend)
  BOX 2 → BOX 3 (down-left bend)
  BOX 3 → BOX 4 (down-left)
  BOX 3 → BOX 5 (down-right)
  BOX 4 → BOX 6 (down-right)
  BOX 5 → BOX 6 (down-left)
  BOX 6 → BOX 7 (horizontal right)

Small caption at top (x: 100-1100, y: 30), centered, 14px Inter Medium #6C7086:
  "data flow · two dirs → picker → fzf+preview → resume"

Small footer at bottom (y: 780), centered, 12px Inter Regular #585B70:
  "diagram · claude-picker v1.2"

Mood: clean, Excalidraw-polished, precise, dark, developer-documentation-grade.

NEGATIVE: no emoji, no 3D, no shadows on boxes, no gradient fills, no photorealism, no hand-drawn wobble, no isometric perspective, no human figures, no decorative icons, no color outside Catppuccin Mocha.
```

---

## 5. Open Graph / LinkedIn Share — 1200x630 — LinkedIn, Slack, Discord, Facebook unfurls

**Where it's used:** `og:image` meta tag for the project landing page / Medium article. Drives the unfurl preview.

**Safe zone:** Central **960x480** with 120 px side margins and 75 px top/bottom margins. LinkedIn crops ~40 px from the top in some views.

**Background:** Flat `#11111B` (Crust) with a **very faint** typographic watermark — the word "claude-picker" in 220px JetBrains Mono Bold at color `#CDD6F4` 4% opacity, rotated 0°, centered but offset left by 80px, clipped by the canvas.

**Prompt:**

```
1200x630 typographic social sharing card. Flat background #11111B. Very subtle watermark: the word "claude-picker" in 220px JetBrains Mono Bold, color #CDD6F4 at 4% opacity, centered horizontally offset 80px left, clipped by canvas edges.

CENTER STACK (vertically centered at y=315, horizontally centered):

Line 1 (eyebrow, 14px Inter Bold, letter-spacing 0.18em, color #CBA6F7):
  "DEVELOPER TOOLS"

Gap 20px.

Line 2 (headline, 54px Inter SemiBold, letter-spacing -0.015em, color #CDD6F4, max 2 lines):
  "Stop clicking through UUIDs."

Gap 16px.

Line 3 (subhead, 22px Inter Regular, color #FAB387, max 2 lines):
  "Browse, preview, and resume Claude Code sessions from your terminal."

Gap 32px.

Line 4 (byline row, centered): horizontal row of three elements separated by 10px dot bullets (#6C7086):
  "github.com/anshul-garg27/claude-picker" in 14px JetBrains Mono #CBA6F7
  "·"
  "MIT licensed" in 14px Inter Regular #6C7086
  "·"
  "v1.2" in 14px JetBrains Mono #A6E3A1

BOTTOM-LEFT CORNER (x: 60, y: 570):
12px Inter Regular #585B70: "by Anshul Garg"

BOTTOM-RIGHT CORNER (x: 1140, y: 570, right-aligned):
12px JetBrains Mono #585B70: "og · 1200×630"

Mood: confident, editorial, typographic, conference-talk-title-card.

NEGATIVE: no emoji, no terminal mockup, no gradient, no glow, no 3D, no stock photos, no human figures, no icons other than the text dot bullets.
```

---

## 6. Project Picker Screenshot — 1200x600 — Docs "Step 1", Medium inline

**Where it's used:** The `## Usage` section step-1 screenshot. Shows what users see first when they run `claude-picker`.

**Safe zone:** Terminal occupies central **1060x520** zone, 70 px horizontal margin, 40 px top/bottom.

**Background:** Flat `#11111B` (Crust) outside the terminal. Optional: a single `#CBA6F7` halo at 10% opacity, 140 px blur, behind the terminal.

**Prompt:**

```
1200x600 terminal screenshot-style image on flat #11111B background. Single subtle mauve halo #CBA6F7 at 10% opacity, 140px blur, behind the terminal.

TERMINAL WINDOW (x: 70-1130, y: 40-560):
1060x520 px, rounded corners 12px, fill #181825, 1px border #45475A.
Chrome bar 32px tall, fill #1E1E2E, three 10px circles left (#F38BA8, #F9E2AF, #A6E3A1). Tab label centered: "claude-picker" in 12px JetBrains Mono #6C7086.

INSIDE TERMINAL, 24px padding, 17px JetBrains Mono, 28px line-height:

Line 1 (header row, two columns): 
  Left: "claude-picker" in bold #CBA6F7 at 19px.
  Right: "enter · ctrl-d delete · esc quit" in 13px #6C7086.

Line 2 (blank).

Line 3 (fzf label, inline with the picker border): "── projects ──" in 14px #6C7086 (styled as a top-border label on a framed box below).

LINES 4-10 inside a subtle 1px #313244 framed box (the fzf labeled border):

Line 4: "▸ " in #CBA6F7 + "architex" in 18px bold #89B4FA + right-aligned "just now   5 sessions" (timestamp #A6E3A1, "5 sessions" #6C7086) + a green activity bar "█████" in #A6E3A1 between name and timestamp.
Line 5: "  " + "ecommerce-api" in 18px bold #89B4FA + right-aligned "2m · 3 sessions" with bar "███" in #A6E3A1.
Line 6: "  " + "infra-automation" in 18px bold #89B4FA + right-aligned "1h · 2 sessions" with bar "██" in #94E2D5.
Line 7: "  " + "portfolio-site" in 18px bold #89B4FA + right-aligned "3h · 2 sessions" with bar "██" in #94E2D5.
Line 8: "  " + "claude-picker" in 18px bold #89B4FA + right-aligned "1d · 4 sessions" with bar "████" in #FAB387.
Line 9: "  " + "old-playground" in 18px bold #89B4FA + right-aligned "6d · 1 session" with bar "█" in #6C7086.
Line 10: blank.

Line 11 (outside box, prompt): "project >" in #89DCEB bold at 16px, blinking-cursor █ in #CDD6F4.

Line 12 (status bar): "6/6" in #6C7086 + "  ·  " + "↑/↓ navigate  enter select  /search" in #6C7086.

Mood: clean, realistic terminal screenshot, developer-grade, Warp-inspired.

NEGATIVE: no emoji, no gradient on terminal, no 3D, no drop shadow on the terminal window (only the halo), no photorealism, no lorem ipsum, no garbled monospace alignment.
```

---

## 7. Session Picker + Preview Panel — 1200x600 — Docs "Step 2", Medium inline

**Where it's used:** Shows the second screen — sessions with the Rich-formatted preview panel to the right.

**Safe zone:** Terminal occupies central **1060x520**.

**Background:** Flat `#11111B`. Same halo treatment as Image 6.

**Prompt:**

```
1200x600 terminal screenshot-style image on flat #11111B. Mauve halo #CBA6F7 at 10% opacity, 140px blur behind the terminal.

TERMINAL (1060x520, rounded 12px, fill #181825, 1px border #45475A, chrome bar with three colored circles).

INSIDE, split 60/40 vertical. A 1px vertical divider #45475A at x=636 (relative to terminal interior).

LEFT 60% (session list, 20px padding, 16px JetBrains Mono, 24px line-height):
  Header row: "architex" in bold #CBA6F7 at 17px + right-aligned "enter · ctrl-d del · ctrl-b pin" in 12px #6C7086.
  Blank.
  "── named ──" in #6C7086 (styled as fzf border label).
  "▸ ● auth-refactor            5m   45 msgs  $0.42" (pointer #CBA6F7, dot #F9E2AF, name bold #A6E3A1, timestamp #A6E3A1 because <1h, msg count #6C7086, cost #A6E3A1).
  "  ● fix-race-condition       2h   28 msgs  $0.18" (timestamp #F9E2AF for 1-6h range).
  "  ● drizzle-migration        1d   67 msgs  $1.05" (timestamp #FAB387 for >6h).
  "  📌 feat-billing            3d   92 msgs  $2.14" — REPLACE the emoji with a mauve pin glyph: a small solid triangle + rectangle shape in #CBA6F7 drawn as "▼" rotated, sized 14px. (Use Unicode "⚑" at color #CBA6F7 if the generator rejects custom glyph.) Timestamp #F38BA8 for >1d.
  Blank.
  "── recent ──" in #6C7086.
  "  ○ session                  4h   12 msgs  $0.08" (all #6C7086 for unnamed).
  "  ○ session                  1d   31 msgs  $0.44" (all #6C7086).
  Blank.
  Prompt: "session >" in #89DCEB bold + blinking cursor █ in #CDD6F4.

RIGHT 40% (preview panel, 18px padding, 14px JetBrains Mono, 22px line-height):
  Title: "auth-refactor" in bold #A6E3A1 at 16px.
  Meta line: "2026-04-14 14:30 · 45 msgs · $0.42" in #6C7086 at 12px.
  Horizontal rule: "─" repeated in #45475A.
  Blank.
  "you" in bold #89B4FA + " the auth middleware is storing session tokens in plain cookies, we need httpOnly + signed" — wrapped to fit.
  Blank.
  "ai" in bold #F9E2AF + " I'll restructure to use encrypted httpOnly cookies with a rotating signing key. First, the middleware:" — wrapped.
  A 3-line inline code block, fill #313244, padding 6px, 12px JetBrains Mono #CDD6F4:
    "res.cookie('sid', encrypt(token), {"
    "  httpOnly: true, secure: true, signed: true"
    "})"
  Blank.
  "you" in bold #89B4FA + " also need refresh token rotation" — one line.
  Blank.
  At the very bottom of the panel, dimmed: "... 42 more messages" in #585B70 italic.

Mood: clean, realistic, developer-grade, Rich-library-rendered-preview look.

NEGATIVE: no emoji (the pin glyph is a stylized mauve shape, not an emoji), no gradients, no drop shadows, no 3D, no lorem ipsum, no mis-aligned monospace columns.
```

---

## 8. `--search` Full-Text Search — 1200x600 — Feature spotlight (search across all projects)

**Where it's used:** `## Search` section of README, Medium inline, Twitter feature card.

**Safe zone:** Central **1060x520**.

**Background:** Flat `#11111B` with a single `#89B4FA` (Blue) halo at 10% opacity, 140 px blur — blue instead of mauve to subtly signal "search mode."

**Prompt:**

```
1200x600 terminal screenshot. Flat #11111B background. Blue halo #89B4FA at 10% opacity, 140px blur.

TERMINAL (1060x520, rounded 12px, fill #181825, 1px border #45475A, chrome dots).

INSIDE, 24px padding, 16px JetBrains Mono, 24px line-height:

Header row:
  Left: "claude-picker --search" in bold #CBA6F7.
  Right: "3 projects · 847 sessions · 12 matches" in #6C7086.

Blank.

Search bar row (fzf-style with a labeled top border "── search ──" #6C7086 on a 1px #313244 framed box):
  "> authentication" in 18px bold #89B4FA, with blinking cursor █ in #CDD6F4, and right-aligned badge "12/847" in 12px pill #313244 with #BAC2DE text.

Blank.

Results label: "── matches ──" in #6C7086 as fzf border label.

Result rows (each shows project name, session name, matched excerpt with highlighted term):

Row 1:
  Line A: "▸ " #CBA6F7 + "[architex]" in 13px #7F849C + " auth-refactor" in bold #A6E3A1 + right-aligned "5m · 45 msgs" #6C7086.
  Line B (excerpt, indent 4 chars, 13px): "...the " + "authentication" highlighted with yellow background #F9E2AF and black text #1E1E2E with 2px padding + " middleware is storing session tokens..." in #BAC2DE.

Row 2:
  Line A: "  [architex] login-rework" in same format, "2d · 34 msgs" timestamp #FAB387.
  Line B: "...refactor " + "authentication" highlighted + " flow to use OAuth2 PKCE..." in #BAC2DE.

Row 3:
  Line A: "  [ecommerce-api] user-service" — "1w · 67 msgs" timestamp #F38BA8.
  Line B: "...JWT-based " + "authentication" highlighted + " with refresh token rotation..." in #BAC2DE.

Row 4:
  Line A: "  [infra-automation] deploy-secrets" — "2w · 23 msgs" timestamp #F38BA8.
  Line B: "...vault-based " + "authentication" highlighted + " for service-to-service..." in #BAC2DE.

Row 5:
  Line A: "  [architex] middleware-audit" — "3w · 89 msgs" #F38BA8.
  Line B: "...audit log emitted on every " + "authentication" highlighted + " attempt..." in #BAC2DE.

Blank.

Footer row: "↑/↓ navigate  enter open  ctrl-g group-by-project  esc close" in 13px #6C7086.

Mood: precise, fast, searchlight-on-data, Algolia-esque confidence.

NEGATIVE: no emoji, no gradient, no magnifying-glass icon inside the terminal (the search is textual), no 3D, no drop shadows, no lorem ipsum, no inconsistent match-highlight colors.
```

---

## 9. `--stats` Dashboard — 1200x800 — Feature spotlight (analytics)

**Where it's used:** `## Stats` section, blog feature callout, LinkedIn post hero.

**Safe zone:** Central **1060x720** (70 px horizontal margin, 40 px vertical).

**Background:** Flat `#11111B`. No halo (dashboard is dense; glow competes with data).

**Prompt:**

```
1200x800 terminal dashboard screenshot. Flat #11111B background. No halo.

TERMINAL (1060x720, rounded 12px, fill #181825, 1px border #45475A, chrome dots).

INSIDE, 24px padding, 15px JetBrains Mono, 22px line-height. A grid of panels, each framed with 1px #313244 and a top-border fzf-style label.

HEADER (full width, 60px tall):
  Left: "claude-picker --stats" in 20px bold #CBA6F7.
  Right: "last 30 days · all projects" in 13px #6C7086.

ROW 1: three summary tiles side-by-side, each 328x110, 12px gap between, rounded 8px, fill #1E1E2E, 1px border #313244, top-label "── tokens ──" / "── cost ──" / "── sessions ──" in 11px #6C7086.

  Tile 1 "── tokens ──":
    Big number "14.2M" in 42px JetBrains Mono Bold #CDD6F4, centered-left.
    Below: "8.1M input · 6.1M output" in 12px #6C7086.
    Right side: inline sparkline in #94E2D5, 12-point, 40px tall, trending up.

  Tile 2 "── cost ──":
    "$127.48" in 42px JetBrains Mono Bold #A6E3A1.
    Below: "avg $4.25 / day" in 12px #6C7086.
    Right: sparkline in #A6E3A1 trending up.

  Tile 3 "── sessions ──":
    "847" in 42px JetBrains Mono Bold #F9E2AF.
    Below: "62 named · 785 unnamed" in 12px #6C7086.
    Right: sparkline in #F9E2AF, flat-ish.

ROW 2: per-project bar chart panel, full width (1012x240), top-label "── per project ──" in #6C7086.

  Left column (name, fixed width 220px): project name in 14px JetBrains Mono Bold #89B4FA.
  Middle column (bar): horizontal bar, max-width 540px, height 20px, fill gradient-free solid color per row, background track #313244 at 30% opacity. Bar color: #A6E3A1 for cost quartile top, #94E2D5 for mid, #F9E2AF for low, #FAB387 for bottom. (Use a single quantized color per bar based on the data; the chart reads as solid, not gradient.)
  Right column (value, 220px, right-aligned): "$xx.xx · yyk tok · zz ses" in 13px #6C7086.

  Rows (6 projects):
    "architex"          bar 92% #A6E3A1   "$47.20 · 4.8M tok · 213 ses"
    "ecommerce-api"     bar 74% #A6E3A1   "$38.10 · 3.9M tok · 187 ses"
    "infra-automation"  bar 52% #94E2D5   "$26.80 · 2.6M tok · 142 ses"
    "portfolio-site"    bar 30% #F9E2AF   "$15.30 · 1.5M tok · 98 ses"
    "claude-picker"     bar 16% #FAB387   "$ 8.20 · 0.9M tok · 82 ses"
    "old-playground"    bar  6% #FAB387   "$ 3.10 · 0.4M tok · 41 ses"

ROW 3: daily timeline panel, full width (1012x200), top-label "── activity (30d) ──" in #6C7086.
  X-axis: 30 day-ticks along the bottom in 10px #585B70, labels every 5 days: "Mar 17", "Mar 22", "Mar 27", "Apr 1", "Apr 6", "Apr 11", "Apr 16".
  Y-axis: 4 horizontal gridlines 1px #313244 at 20% opacity.
  Data: 30 vertical bars, 18px wide, 8px gap. Bar color #CBA6F7 for most, with two red bars #F38BA8 at days 21 and 27 (high-cost days), and the last 3 bars in brighter #A6E3A1 (ramp-up trend).
  Small annotation at day 21: "← ouch" in 10px italic #F38BA8.
  Small annotation at last bar: "today ↑" in 10px italic #A6E3A1.

FOOTER: "press q to quit · press e to export · press t to toggle days/weeks" in 12px #6C7086.

Mood: dense, confident, data-forward, ink-on-dark-paper, Stripe-dashboard-quiet.

NEGATIVE: no emoji, no 3D bars, no gradient fills on bars, no pie charts, no photographs, no animated-looking glows, no decorative backgrounds, no stock icons. All numbers must be consistent with each other (sessions and tokens totals tie to the row 1 tiles).
```

---

## 10. `--tree` Session Tree — 1200x800 — Feature spotlight (forks & branches)

**Where it's used:** `## Tree` section of README, Medium inline when explaining fork relationships.

**Safe zone:** Central **1060x720**.

**Background:** Flat `#11111B`.

**Prompt:**

```
1200x800 terminal tree-view screenshot. Flat #11111B background. Single mauve halo #CBA6F7 at 8% opacity, 160px blur behind the terminal.

TERMINAL (1060x720, rounded 12px, fill #181825, 1px border #45475A, chrome dots).

INSIDE, 24px padding, 15px JetBrains Mono, 26px line-height (extra breathing room for tree).

Header:
  Left: "claude-picker --tree" in 20px bold #CBA6F7.
  Right: "architex · 12 sessions · 3 forks" in 13px #6C7086.

Blank.

Fzf-style top-border label: "── session tree ──" in #6C7086.

Tree content. Use Unicode box-drawing: "├──", "│", "└──", "┬──". All tree connectors in #45475A. Session names as specified. Each node: dot glyph + name + timestamp + msg count + optional cost.

  ● main                                2w   124 msgs  $3.80   [#A6E3A1 bold name, #F38BA8 timestamp]
  ├── ● auth-refactor                   5m    45 msgs  $0.42   [#A6E3A1, #A6E3A1 timestamp — fresh]
  │   ├── ● auth-refactor-oauth         2h    18 msgs  $0.14   [#A6E3A1, #F9E2AF timestamp — forked 2h ago]
  │   └── ● auth-refactor-sessions      1h    22 msgs  $0.19   [#A6E3A1, #F9E2AF]
  ├── ● payment-gateway                 2h    28 msgs  $0.18   [#A6E3A1, #F9E2AF]
  │   └── ● payment-stripe-retry        45m   14 msgs  $0.09   [#A6E3A1, #F9E2AF]
  ├── ● drizzle-migration               1d    67 msgs  $1.05   [#A6E3A1, #FAB387]
  ├── ● k8s-deployment                  3d    52 msgs  $0.89   [#A6E3A1, #F38BA8]
  │   ├── ● k8s-deployment-helm         2d    19 msgs  $0.22   [#A6E3A1, #F38BA8]
  │   └── ● k8s-deployment-kustomize    2d    24 msgs  $0.31   [#A6E3A1, #F38BA8]
  └── ● logs-overhaul                   5d    38 msgs  $0.51   [#A6E3A1, #F38BA8]

Legend row at bottom, 12px #6C7086:
  "● named   ○ unnamed   │├──└── fork  ·  colors: fresh <1h · today <6h · day <1d · older >1d"

Blank.

Footer: "↑/↓ navigate  enter open  ctrl-f show-fork-origin  esc close" in 12px #6C7086.

Mood: structural, calm, version-controlled, git-log-but-readable.

NEGATIVE: no emoji, no actual git commit hashes visible, no gradient, no 3D, no decorative illustrations, no color outside Catppuccin Mocha, all tree connectors must align perfectly vertically (no jitter).
```

---

## 11. `--diff` Side-by-Side Comparison — 1400x800 — Feature spotlight (two-session compare)

**Where it's used:** `## Diff` section, Medium inline, YouTube thumbnail variant.

**Safe zone:** Central **1260x720**.

**Background:** Flat `#11111B`.

**Prompt:**

```
1400x800 terminal diff screenshot. Flat #11111B background. No halo (density).

TERMINAL (1260x720, rounded 12px, fill #181825, 1px border #45475A, chrome dots). Tab label: "architex · diff auth-refactor ↔ auth-refactor-oauth" in 12px #6C7086.

INSIDE, 24px padding, 14px JetBrains Mono, 22px line-height.

Header row:
  Left: "claude-picker --diff" in 20px bold #CBA6F7.
  Right: "2 sessions · 22 divergent turns" in 13px #6C7086.

Blank.

Two-column diff layout with a 1px vertical divider #45475A at the exact center (x=606 relative to terminal interior).

LEFT COLUMN HEADER (y: 60, fzf-style top-border label):
  "── auth-refactor ──" in 13px #A6E3A1.
  Below (13px, #6C7086): "5m · 45 msgs · $0.42 · model: opus-4.7"

RIGHT COLUMN HEADER:
  "── auth-refactor-oauth ──" in 13px #A6E3A1.
  Below: "2h · 18 msgs · $0.14 · model: opus-4.7 · forked from ↑"

Both columns: vertical stream of message rows. Each row has a role badge, a body, and a 3px left accent bar indicating diff status.

Row structure for both columns:
  Role badge: "you" in #89B4FA bold 12px, or "ai" in #F9E2AF bold.
  Body in 12px #CDD6F4, wrapped, max 3 lines before ellipsis.
  Left accent bar (3px wide, 100% row height):
    #A6E3A1 = same in both (unchanged, present in both)
    #F9E2AF = modified (similar role turn but different content)
    #A6E3A1-with-plus-sign = added (only in this column; "+ " prefix)
    #F38BA8-with-minus-sign = removed (only in other column; the other col shows "- " prefix with #F38BA8 bar and strikethrough body)

Actual content (imagined realistic turns):

LEFT column rows:
  "you: the auth middleware is storing session tokens in plain cookies..." [#A6E3A1 bar, same]
  "ai: I'll restructure to use encrypted httpOnly cookies..." [#A6E3A1, same]
  "you: also need refresh token rotation" [#A6E3A1, same]
  "ai: using a Redis-backed rotation table, let's define the schema..." [#F9E2AF bar, modified — this turn diverges]
  "you: yes, Redis with 7-day TTL on refresh tokens" [#F9E2AF, modified]
  "ai: here's the final middleware with Redis rotation:" [#F9E2AF, modified]
  "[code block: 8 lines of node.js middleware]" [#F9E2AF, modified]
  "you: perfect, add logging" [#A6E3A1, same]
  "ai: added structured logs with pino..." [#A6E3A1, same]
  "you: run the tests" [#A6E3A1, same]
  "(removed row) --- [#F38BA8 bar, strikethrough #585B70] 'you: let's switch to oauth' — shown greyed"

RIGHT column rows:
  "you: the auth middleware is storing session tokens in plain cookies..." [#A6E3A1 same]
  "ai: I'll restructure to use encrypted httpOnly cookies..." [#A6E3A1 same]
  "you: also need refresh token rotation" [#A6E3A1 same]
  "ai: let's drop custom rotation and use OAuth2 with PKCE via an identity provider..." [#F9E2AF modified]
  "you: keycloak or auth0?" [#F9E2AF modified]
  "ai: auth0 for speed, keycloak for control. Given your traffic, auth0's free tier fits..." [#F9E2AF modified]
  "[code block: 6 lines of auth0 SDK init]" [#F9E2AF modified]
  "(added row) + 'you: what about MFA?' [#A6E3A1 with + prefix]"
  "(added row) + 'ai: auth0 supports TOTP + WebAuthn out-of-box...' [#A6E3A1 with + prefix]"
  "you: perfect, add logging" [#A6E3A1 same]
  "ai: added structured logs with pino..." [#A6E3A1 same]

FOOTER (y: 760):
  Left: "12 unchanged · 8 modified · 2 added · 1 removed" in 12px #6C7086.
  Right: "↑/↓ scroll  j/k jump-diff  enter open-session  esc close" in 12px #6C7086.

Mood: surgical, precise, GitHub-diff-for-conversations, quiet.

NEGATIVE: no emoji, no green/red background fills on whole rows (only 3px left bars), no git-raw "+++" "---" full-line markers, no photorealistic UI chrome, no 3D, no lorem ipsum.
```

---

## 12. Bookmarks (Ctrl+B) — 1200x630 — Feature spotlight (pinning)

**Where it's used:** Feature card for the bookmarks flow.

**Safe zone:** Central **1040x520**.

**Background:** Flat `#11111B`, with a soft `#89B4FA` halo at 10% opacity (blue to signal "saved / pinned").

**Prompt:**

```
1200x630 feature spotlight image. Flat #11111B background. Blue halo #89B4FA at 10% opacity, 140px blur.

LEFT HALF (x: 60-560, vertically centered):
Eyebrow (14px Inter Bold, letter-spacing 0.18em, #89B4FA): "BOOKMARKS"
Headline (44px Inter SemiBold, -0.015em, #CDD6F4): "Pin what you'll return to."
Subhead (18px Inter Regular, #A6ADC8): "Press Ctrl+B on any session. Bookmarked sessions float to the top across every picker launch."
Keybinding badge row (28px tall each, 8px radius, fill #313244, 1px border #45475A, 13px JetBrains Mono inside #BAC2DE):
  "Ctrl+B pin"   "Ctrl+Shift+B unpin"   "/bookmarked filter"

RIGHT HALF (x: 620-1160):
Terminal mockup 540x440, rounded 12px, fill #181825, 1px border #45475A, chrome dots. Tab: "architex — bookmarks".

Inside, 18px padding, 14px JetBrains Mono, 24px line-height:

Header:
  "claude-picker" in bold #CBA6F7 + "  architex" in #89B4FA.

Fzf label: "── pinned ──" in #6C7086.

Pinned rows (each prefixed with a stylized mauve pin glyph "⚑" in #CBA6F7):
  "⚑ auth-refactor           5m   45 msgs  $0.42" [pin #CBA6F7, name #A6E3A1 bold, timestamp #A6E3A1, meta #6C7086]
  "⚑ k8s-deployment          3d   52 msgs  $0.89" [pin #CBA6F7, name #A6E3A1, timestamp #F38BA8]
  "⚑ feat-billing            1w   92 msgs  $2.14" [pin #CBA6F7, timestamp #F38BA8]

Fzf label: "── named ──" in #6C7086.
  "▸ ● payment-gateway       2h   28 msgs  $0.18"
  "  ● drizzle-migration     1d   67 msgs  $1.05"
  "  ● logs-overhaul         5d   38 msgs  $0.51"

Fzf label: "── recent ──" in #6C7086.
  "  ○ session               4h   12 msgs"
  "  ○ session               1d   31 msgs"

Prompt: "session >" in #89DCEB.

Tiny floating toast in the bottom-right of the terminal: a rounded pill, fill #1E1E2E, 1px border #89B4FA, 8px padding, 12px text #89B4FA: "⚑ pinned auth-refactor".

Mood: reassuring, organized, blue-signals-save.

NEGATIVE: no emoji (the pin is a stylized Unicode/vector glyph), no push-pin stock icon, no gradient, no 3D.
```

---

## 13. Export to Markdown (Ctrl+E) — 1200x630 — Feature spotlight (export)

**Where it's used:** Feature card for the markdown export flow.

**Safe zone:** Central **1040x520**.

**Background:** Flat `#11111B`, soft `#F9E2AF` (Yellow) halo at 8% opacity, 140 px blur (yellow signals "export → file").

**Prompt:**

```
1200x630 feature spotlight. Flat #11111B background. Yellow halo #F9E2AF at 8% opacity, 140px blur.

TWO-STAGE LAYOUT with a small arrow in the center.

LEFT STAGE (x: 60-560): a terminal mockup 500x440, rounded 12px, fill #181825.
  Header: "claude-picker" in bold #CBA6F7.
  Fzf label: "── named ──"
  "▸ ● auth-refactor           5m   45 msgs" [pointer #CBA6F7, name #A6E3A1 bold]
  "  ● payment-gateway         2h   28 msgs"
  "  ● drizzle-migration       1d   67 msgs"
  A highlighted status row at bottom of terminal: a fill #313244 pill with text "ctrl-e exporting auth-refactor..." in 13px #F9E2AF + an animated 3-dot indicator "..." in #F9E2AF (drawn static but spaced).
  Bottom toast: "✓ wrote ~/Desktop/auth-refactor.md (47 KB)" in 12px #A6E3A1 with a thin #A6E3A1 left bar.

CENTER ARROW (x: 560-640, vertically centered):
A large → arrow in 60px Inter Light #CBA6F7, gently offset y with a tiny "ctrl·e" label above in 10px #6C7086.

RIGHT STAGE (x: 640-1160): a "file card" visual representing the exported markdown — 480x440, rounded 12px, fill #181825, 1px border #F9E2AF.
  Top: a "file name bar" 40px tall, fill #1E1E2E, text "auth-refactor.md" in 14px JetBrains Mono Bold #F9E2AF left-aligned, small "47 KB · 45 msgs" in 12px #6C7086 right-aligned.
  Body: a dim markdown preview with realistic rendering. 13px in mix of sans/mono. Sample lines:
    "# auth-refactor" (as 16px Inter Bold #CDD6F4)
    "_architex · 2026-04-16 · 45 messages · $0.42_" (13px italic #6C7086)
    ""
    "## Turn 1"
    "**you:** the auth middleware is storing session tokens in plain cookies..."
    "**ai:** I'll restructure to use encrypted httpOnly cookies..."
    "```js"
    "res.cookie('sid', encrypt(token), {"
    "  httpOnly: true, secure: true, signed: true"
    "})"
    "```"
    "## Turn 2"
    "**you:** also need refresh token rotation"
    "**ai:** using a Redis-backed rotation table..."
    Fade the bottom 20% of the body into #181825 to suggest continuation.

SUPER-SMALL CAPTION under the whole layout, centered (y=600):
  "Ctrl+E · export any session to a portable .md file" in 12px #6C7086.

Mood: productive, quiet satisfaction, "it just wrote the file."

NEGATIVE: no emoji, no file-explorer chrome, no OS-specific widgets, no Finder icon, no 3D paper curl, no gradient.
```

---

## 14. Cost Tracking Highlight — 1200x675 — Feature spotlight (cost / token per session)

**Where it's used:** Twitter card, feature section, newsletter banner when talking about cost transparency.

**Safe zone:** Central **1040x575**.

**Background:** Flat `#11111B`, soft `#A6E3A1` halo at 7% opacity, 140 px blur (green signals money).

**Prompt:**

```
1200x675 feature spotlight. Flat #11111B background. Green halo #A6E3A1 at 7% opacity, 140px blur behind the terminal.

LEFT HALF (x: 60-560, vertically centered):
Eyebrow (14px Inter Bold, letter-spacing 0.18em, #A6E3A1): "COST TRACKING"
Headline (42px Inter SemiBold, -0.015em, #CDD6F4): "Every session shows what it cost."
Subhead (18px Inter Regular, #A6ADC8): "Tokens in, tokens out, dollars spent. Computed from the local JSONL — no API calls required."
Three stat chips stacked (each 32px tall, fill #1E1E2E, 1px border #313244, 14px inside):
  "↑ $127.48 spent this month" (arrow #FAB387, number #A6E3A1 bold, rest #A6ADC8)
  "↑ 14.2M tokens processed"
  "↑ 847 sessions tracked"

RIGHT HALF (x: 620-1160):
Terminal mockup 540x515, rounded 12px, fill #181825, 1px border #45475A, chrome dots.

Inside, 18px padding, 15px JetBrains Mono, 26px line-height:

Header:
  "claude-picker" in bold #CBA6F7 + "  architex" in #89B4FA + right-aligned "sort: cost↓" in 12px #6C7086.

Fzf label: "── by cost ──" in #6C7086.

Rows (sorted by cost descending, each row shows name, timestamp, msg count, and the cost emphasized):
  "▸ ● feat-billing              1w    92 msgs   $2.14" [pointer #CBA6F7, name #A6E3A1 bold, timestamp #F38BA8, meta #6C7086, cost in bold #A6E3A1 17px]
  "  ● drizzle-migration         1d    67 msgs   $1.05" [cost bold #A6E3A1]
  "  ● k8s-deployment            3d    52 msgs   $0.89" [timestamp #F38BA8]
  "  ● logs-overhaul             5d    38 msgs   $0.51" [timestamp #F38BA8]
  "  ● auth-refactor             5m    45 msgs   $0.42" [timestamp #A6E3A1]
  "  ● payment-gateway           2h    28 msgs   $0.18" [timestamp #F9E2AF]
  "  ● payment-stripe-retry     45m    14 msgs   $0.09" [timestamp #F9E2AF]
  "  ○ session                   4h    12 msgs   $0.08"
  "  ○ session                   1d    31 msgs   $0.44"

Blank.

Totals row (bold, contrasting fill #1E1E2E strip across the full terminal width):
  "total  · 9 visible  ·  379 msgs  ·  $5.80" in 14px JetBrains Mono Bold #CDD6F4. Cost figure colored #A6E3A1.

Prompt: "session >" in #89DCEB.

Mood: transparent, calm, accountant-grade, money-is-sacred.

NEGATIVE: no emoji, no dollar-bill icon, no gradient, no 3D currency, no stock finance imagery.
```

---

## 15. Logo / Icon — 512x512 — GitHub avatar, npm avatar, social profile, favicon source

**Where it's used:** Save as `assets/ai-generated/github/logo.png`. Use for GitHub org avatar, npm package icon, social profile images.

**Safe zone:** The icon glyph lives inside a central **360x360** area. The outer 76 px on each side is breathing room for circular masks (GitHub/avatar crops).

**Background:** Option A (default): flat `#1E1E2E`. Option B (transparent variant): no background, export PNG with alpha.

**Prompt:**

```
512x512 minimal geometric icon on flat #1E1E2E background (also export a transparent-background variant).

The icon combines two visual metaphors into one glyph:
1. A magnifying glass (find / pick)
2. A terminal chevron "›" (CLI)

DESIGN:
- A circle (the magnifying-glass lens) positioned upper-left-center. Diameter 180px. Stroke 18px, color #CBA6F7 (mauve). Fill transparent (empty lens). Inside the circle, nothing — or optionally a tiny centered 6px filled dot at color #1E1E2E (barely visible) to reinforce "lens."
- The magnifying-glass "handle" is replaced by the chevron symbol "›" — a stylized angle bracket formed by two 18px strokes meeting at 90° (actually ~110° to feel organic), angled from lower-right to upper-left, starting at the edge of the lens circle at the 4-5 o'clock position and extending toward the bottom-right corner. Color #CBA6F7.
- The tip of the chevron falls at roughly x=360, y=380 (relative to a 512x512 canvas with center 256,256).

PROPORTIONS:
- Lens center: (210, 200).
- Lens radius: 90px.
- Chevron start: (268, 258) (tangent to lens at 45°).
- Chevron mid-angle point: (340, 340).
- Chevron end tip: (380, 380).

STROKE STYLE:
- Rounded line caps.
- Rounded joins.
- Uniform stroke width 18px.
- Single color #CBA6F7. No gradient, no second color.

NO TEXT. No shadow. No glow. Pure vector-feeling geometric mark.

The glyph must read recognizably at 16x16px (favicon size). Test mental-downsample: does the lens+chevron still read as one shape? If not, thicken stroke to 22px.

Style reference: Raycast icon, Linear icon, Warp icon — clean geometric marks, one shape, one color, dark background.

NEGATIVE: no text, no wordmark, no emoji, no photorealism, no 3D, no gradient, no drop shadow, no glow, no second color, no background texture, no frame, no border. Do not draw a literal terminal window, do not draw the word "picker," do not add a cursor blinking bar.
```

---

## 16. Favicon Variant — 64x64 — Docs site favicon, browser tab

**Where it's used:** `favicon.ico`, `apple-touch-icon`, browser tab, bookmark icon.

**Safe zone:** Glyph within central **48x48**.

**Background:** Flat `#1E1E2E` (or transparent, both exports needed).

**Prompt:**

```
64x64 simplified favicon derived from the main logo. Flat #1E1E2E background, also export transparent-bg variant.

DESIGN:
- Simplified lens + chevron. Lens circle centered at (26, 24), radius 16px, stroke 5px, color #CBA6F7.
- Chevron from (36, 32) to (50, 52), single stroke 5px, rounded caps, color #CBA6F7.
- No text. No decoration.

Pixel-snap the strokes so at 16x16 (standard favicon downsample), the lens still reads as a circle (not a blob) and the chevron still reads as a line (not a dot).

Export sizes: 16x16, 32x32, 48x48, 64x64, all from the same vector. Also export 180x180 apple-touch-icon.

Style: identical logic to the main logo, simpler proportions. Raycast/Linear favicon aesthetic.

NEGATIVE: no text, no emoji, no photo, no gradient, no glow, no second color, no background pattern.
```

---

## 17. Twitter Card (generic) — 1200x675 — Any tweet without a GIF

**Where it's used:** Twitter/X `summary_large_image` card; works as a generic wallpaper for any tweet about the project.

**Safe zone:** Central **960x540**.

**Background:** Flat `#11111B` with a very subtle diagonal stripe pattern — 2px wide stripes every 40px, at angle 12°, color `#CDD6F4` at 2% opacity. Stripes feel like a woven texture, not a pattern.

**Prompt:**

```
1200x675 Twitter card. Flat #11111B base with a subtle diagonal stripe pattern: 2px wide stripes every 40px at 12° angle, color #CDD6F4 at 2% opacity.

TOP-LEFT MARK (x: 60, y: 60):
The logo glyph from Image 15 at 48x48 (mauve #CBA6F7 lens+chevron), followed by the wordmark "claude-picker" in 26px JetBrains Mono Bold #CDD6F4 inline-right of the glyph.

CENTER STACK (centered, y: 200-500):

Headline (56px Inter SemiBold, -0.015em, #CDD6F4, max 3 lines):
  "Stop clicking through UUIDs."

Gap 18px.

Subhead (22px Inter Regular, #FAB387, max 2 lines):
  "Browse, preview, and resume Claude Code sessions from any terminal."

Gap 28px.

Three pill badges in a horizontal row, centered, 32px tall, 16px horizontal padding, 10px radius, fill #313244, 1px border #45475A, 14px JetBrains Mono #BAC2DE inside. 12px gap between pills.
  "bash + python + fzf"    "432 lines"    "MIT"

BOTTOM-RIGHT (x: 1000-1140, y: 605):
"github.com/anshul-garg27/claude-picker" in 13px JetBrains Mono #CBA6F7, right-aligned.

BOTTOM-LEFT (x: 60, y: 605):
"v1.2 · April 2026" in 13px Inter Regular #6C7086.

Mood: confident, announcement-grade, Linear-vibe.

NEGATIVE: no emoji, no terminal mockup on this one (it's the typographic variant), no gradient, no glow, no 3D, no photograph, no human figures.
```

---

## 18. Reddit Thumbnail — 1200x630 — r/commandline, r/ClaudeAI, r/programming

**Where it's used:** Reddit self-post image or link-post thumbnail. Must be readable at 70x70 px in the compact feed.

**Safe zone:** Central **1000x500**. Critical — Reddit shrinks thumbnails brutally.

**Background:** Flat `#1E1E2E`. Max contrast is the priority. No halo, no subtle effects (they vanish at thumbnail size).

**Prompt:**

```
1200x630 Reddit thumbnail. Flat #1E1E2E. No halo, no effects — must stay readable when scaled to 70x70 px thumbnail.

LEFT 45% (x: 60-540):
Terminal mockup 480x440, rounded 10px, fill #181825, 1px border #45475A, chrome dots.
Inside, 20px padding, 18px JetBrains Mono Bold (larger than usual for thumbnail legibility), 30px line-height:

  "claude-picker" in #CBA6F7
  ""
  "▸ ● auth-refactor    5m   45" (pointer #CBA6F7, dot #F9E2AF, name #A6E3A1, rest #6C7086)
  "  ● payment-gw       2h   28"
  "  ● k8s-deploy       1d   67"
  "  ● drizzle-mig      3d   42"
  ""
  "session >" in #89DCEB.

(Note: shorter strings than other images — because at 70x70 px, long strings become indistinguishable noise.)

RIGHT 55% (x: 600-1140, vertically centered):
Headline (56px Inter SemiBold, -0.015em, #CDD6F4, left-aligned):
  "claude-picker"
Below (18px Inter Regular, #A6ADC8):
  "Session manager for Claude Code."
Below (16px JetBrains Mono, #CBA6F7):
  "github.com/anshul-garg27/claude-picker"
Below (14px Inter Regular, #6C7086):
  "bash · python · fzf · MIT"

TOP-RIGHT CORNER (x: 1100, y: 40):
A small 36x36 version of the logo glyph (#CBA6F7 lens+chevron).

Contrast test: at 70x70 px, the wordmark "claude-picker" must still be recognizable, the terminal must still read as "a dark box with colored lines."

Mood: punchy, readable, high-contrast, post-thumbnail-friendly.

NEGATIVE: no emoji, no low-contrast text (<4.5:1 ratio to bg), no gradient, no glow, no small-font-heavy composition, no decorative detail that will be lost at 70x70.
```

---

## 19. Product Hunt Gallery — 1270x760 — PH listing gallery slots 2–5

**Where it's used:** Product Hunt product page gallery images (the carousel below the main demo GIF).

**Safe zone:** Central **1110x660** (80 px margin each side). PH adds a thin border.

**Background:** Flat `#1E1E2E`, with a **single** diagonal accent swoosh — a 120 px wide band of `#CBA6F7` at 8% opacity running from bottom-left to top-right at 18°, behind the content.

**Prompt:**

```
1270x760 Product Hunt gallery card. Flat #1E1E2E base. A single diagonal accent band: 120px wide, 18° angle, running from bottom-left corner to top-right corner, color #CBA6F7 at 8% opacity, no hard edge (soft 40px feather on both sides).

TOP BAR (y: 60, full width, 80px tall):
Left (x=80): small logo glyph 40x40 (#CBA6F7) + wordmark "claude-picker" in 28px JetBrains Mono Bold #CDD6F4.
Right (x=1190, right-aligned): "a session manager for Claude Code" in 16px Inter Regular #A6ADC8.

MAIN AREA (y: 160-700), split 55/45:

LEFT 55% (x: 80-780): terminal mockup 700x540, rounded 12px, fill #181825, 1px border #45475A, chrome dots. Tab: "architex". 
Inside, 22px padding, 16px JetBrains Mono, 26px line-height — use the same content pattern as Image 7 (session picker with preview split). Keep text sharp.

RIGHT 45% (x: 810-1190): three stacked feature cards, each 360x168, 20px gap, rounded 10px, fill #181825, 1px border #45475A, 18px padding.

Card 1 (top):
  Small 18x18 glyph of magnifier in #89B4FA (or simple line-art Unicode "⌕").
  Title: "Fuzzy Search" in 18px Inter SemiBold #CDD6F4.
  Body: "Type to filter. Instant fzf-powered results across all projects." in 13px Inter Regular #A6ADC8.

Card 2 (middle):
  Small 18x18 glyph of eye symbol in #F9E2AF.
  Title: "Live Preview"
  Body: "Read the last turns before opening. No more guessing what 'session' was."

Card 3 (bottom):
  Small 18x18 glyph of dollar mark "$" in #A6E3A1.
  Title: "Cost + Tokens"
  Body: "Every session shows its cost. Stats and tree views included."

FOOTER BAR (y: 720, full width, 30px tall):
Left: "free · open source · MIT" in 12px Inter Regular #6C7086.
Right: "github.com/anshul-garg27/claude-picker" in 12px JetBrains Mono #CBA6F7.

Mood: polished gallery card, deck-slide-grade, Product-Hunt-gallery-winner.

NEGATIVE: no emoji, no 3D cards, no gradient fills (the diagonal band is the only non-flat element), no drop shadows, no stock icons, no human figures, no sticker-pack look.
```

---

## 20. Product Hunt Thumbnail — 240x240 — PH tile, newsletter thumb

**Where it's used:** Product Hunt listing thumbnail, newsletter feed, small grids.

**Safe zone:** Central **200x200**.

**Background:** Flat `#1E1E2E`.

**Prompt:**

```
240x240 square thumbnail tile. Flat #1E1E2E.

Center the logo glyph from Image 15 at 128x128, color #CBA6F7, exactly centered at (120, 100).

Below the glyph, centered at y=180, the wordmark "claude-picker" in 18px JetBrains Mono Bold #CDD6F4.

Below the wordmark at y=206, a tiny tagline "session manager" in 11px Inter Regular #6C7086, centered.

No other elements.

The glyph + wordmark + tagline must read at 60x60 downsample.

Mood: clean, recognizable, tile-optimized.

NEGATIVE: no emoji, no gradient, no glow, no border, no photograph, no 3D.
```

---

## 21. Instagram Story Template — 1080x1920 — Any IG/Threads story slide

**Where it's used:** Generic template. Reusable for any announcement slide.

**Safe zone:** Central **900x1500** (90 px horizontal margin, 210 px top/bottom — IG overlays UI in the top and bottom ~180 px).

**Background:** Flat `#11111B` with the same subtle diagonal stripe pattern as Image 17 (2 px stripes every 40 px, 12°, `#CDD6F4` at 2% opacity).

**Prompt:**

```
1080x1920 vertical Instagram/Threads story template. Flat #11111B with subtle diagonal stripe pattern (2px stripes every 40px at 12°, #CDD6F4 at 2% opacity).

TOP BLOCK (y: 220-380):
Logo glyph 72x72 (#CBA6F7) centered at (540, 256).
Wordmark below (y=340), centered, 30px JetBrains Mono Bold #CDD6F4: "claude-picker".

CENTER BLOCK (y: 580-1340): main message area. Reserve for a headline + optional terminal + subhead. Use the following slot structure:

Slot 1 - Eyebrow (y: 620), centered, 20px Inter Bold letter-spacing 0.18em #CBA6F7:
  "[EYEBROW — e.g., NEW FEATURE]"

Slot 2 - Headline (y: 680-900), centered, 68px Inter SemiBold -0.015em #CDD6F4, max 4 lines:
  "[Headline — large, 2-4 lines of punchy copy.]"

Slot 3 - Optional visual (y: 940-1260): either a centered 720x320 terminal mockup OR a centered graphic. If terminal, use the Image 6 content at scaled-up font (22px mono).

Slot 4 - Subhead (y: 1300), centered, 24px Inter Regular #A6ADC8, max 2 lines:
  "[Subhead line — 1-2 lines of context.]"

BOTTOM CTA BLOCK (y: 1580-1720):
Centered pill: 320px wide, 56px tall, 28px radius, fill #313244, 1px border #CBA6F7, text "github.com/anshul-garg27/claude-picker" in 15px JetBrains Mono Bold #CBA6F7, centered.
Below (y=1700), centered, 13px Inter Regular #6C7086: "tap to copy".

(This is a TEMPLATE. Replace bracketed slots per slide.)

Mood: vertical-native, thumb-stopping, editorial-for-stories.

NEGATIVE: no emoji, no horizontal-layout artifacts (don't waste vertical space), no gradient, no glow, no stock photo, no human figures, no lorem ipsum leaked into final export.
```

---

## 22. LinkedIn Carousel Template — 1080x1350 — Any LinkedIn carousel slide

**Where it's used:** LinkedIn document-post / carousel slides. Reusable template.

**Safe zone:** Central **900x1150**. LinkedIn crops the edges slightly on mobile.

**Background:** Flat `#11111B`. No stripes (LinkedIn compresses textures poorly).

**Prompt:**

```
1080x1350 vertical LinkedIn carousel slide template. Flat #11111B background. No texture.

TOP HEADER (y: 60-160):
Left (x=80): logo glyph 52x52 (#CBA6F7).
Right of glyph: wordmark "claude-picker" in 26px JetBrains Mono Bold #CDD6F4, aligned with glyph baseline.
Far-right (x=1000, right-aligned): slide counter "[SLIDE_N / SLIDE_TOTAL]" in 14px Inter Regular #6C7086.

DIVIDER (y: 180): 1px horizontal line #313244, spanning x=80 to x=1000.

CONTENT AREA (y: 220-1180): flexible slot. Structure:

Slot A - Eyebrow (y=240), 16px Inter Bold 0.18em letter-spacing #CBA6F7:
  "[EYEBROW]"

Slot B - Headline (y=280-480), 56px Inter SemiBold -0.015em #CDD6F4, max 4 lines:
  "[Headline]"

Slot C - Body (y=520-1100), 22px Inter Regular #A6ADC8, max 12 lines:
  "[Body copy — bullets or paragraph.]"

  For bullet slides: each bullet starts with a 6x6 filled square in #CBA6F7 as bullet marker, 14px left-padding, 36px between-bullet gap, 22px bullet text in #CDD6F4.

  For terminal slides: replace Slots B/C with a centered terminal mockup 920x600, rounded 12px, fill #181825, 1px border #45475A, full content per Image 6 conventions.

FOOTER (y: 1240-1310):
Left (x=80): "Anshul Garg · Apr 2026" in 14px Inter Regular #6C7086.
Right (x=1000, right-aligned): "→ Next" in 15px Inter SemiBold #CBA6F7. (Remove on last slide; replace with "github.com/anshul-garg27/claude-picker" in 14px JetBrains Mono #CBA6F7.)

(TEMPLATE. Replace bracketed slots per slide.)

Mood: professional, LinkedIn-appropriate, carousel-native, "swipeable."

NEGATIVE: no emoji, no gradient, no glow, no stock imagery, no selfie/headshot, no corporate-clipart icons, no "hook" emoji arrows.
```

---

## 23. Claude Code Skill Card — 1200x630 — `/claude-picker` skill launch announcement

**Where it's used:** Launch post announcing the Claude Code skill integration. Works on Twitter, LinkedIn, blog header.

**Safe zone:** Central **1000x510**.

**Background:** Flat `#11111B`, soft `#CBA6F7` halo at 10% opacity, 160 px blur, upper-right quadrant (off-center — signals the slash-command appearing).

**Prompt:**

```
1200x630 announcement card. Flat #11111B. Mauve halo #CBA6F7 at 10% opacity, 160px blur, positioned upper-right (center around x=950, y=180).

LEFT HALF (x: 60-560, vertically centered):
Eyebrow (14px Inter Bold, 0.18em letter-spacing, #CBA6F7): "CLAUDE CODE SKILL"
Headline (50px Inter SemiBold, -0.015em, #CDD6F4, max 3 lines): "Type /claude-picker."
Subhead (20px Inter Regular, #A6ADC8, max 3 lines): "Resume any past session from inside Claude Code. No terminal switching. No copy-paste."
Small keybind row: "available in Claude Code 2.3+" in 13px Inter Regular #6C7086.

RIGHT HALF (x: 620-1160):
A stylized Claude Code prompt mockup — NOT a full terminal. Show only the slash-command palette:

A rounded box 540x280, radius 14px, fill #181825, 1px border #45475A, subtle 1px inner highlight on top edge in #313244.

Inside, 22px padding, 17px JetBrains Mono, 28px line-height:

  Prompt row: ">" in #89DCEB bold + " /claude-picker" in #CBA6F7 bold + blinking cursor █ in #CDD6F4.
  Blank.
  A fzf-style dropdown below (width 500, fill #1E1E2E, 1px border #45475A, 8px radius):
    Top-border label: "── 3 commands ──" in 11px #6C7086.
    Row 1 (highlighted, fill #313244, pointer #CBA6F7): "/claude-picker resume       resume a past session" in 15px, command in #CBA6F7, description in #A6ADC8.
    Row 2: "/claude-picker search       full-text across sessions"
    Row 3: "/claude-picker bookmark     pin current session"

Below the box, 13px Inter Regular #6C7086, right-aligned: "ships with claude-picker v1.2"

Mood: launch, announcement, feature-introduction, Claude-Code-native.

NEGATIVE: no emoji (the cursor is a block, not an animated sparkle), no Claude-logo imitations, no Anthropic branding (no C-mark, no official Claude logo), no gradient, no 3D, no stock imagery.
```

---

## 24. Warp Integration Card — 1200x630 — Warp blocks / quick-launch announcement

**Where it's used:** Warp integration announcement post.

**Safe zone:** Central **1000x510**.

**Background:** Flat `#11111B`, soft `#74C7EC` (Sapphire) halo at 8% opacity — cooler signal to differentiate from Claude Code card.

**Prompt:**

```
1200x630 Warp integration announcement card. Flat #11111B. Sapphire halo #74C7EC at 8% opacity, 160px blur, positioned upper-left (center around x=300, y=160).

TOP-LEFT BADGE (x: 60, y: 60):
A small rounded pill, 120x32, fill #1E1E2E, 1px border #74C7EC, text "WARP INTEGRATION" in 12px Inter Bold letter-spacing 0.16em #74C7EC.

CENTER HEADLINE (x: 60-1140, y: 150-320):
"Launch claude-picker from any Warp block." in 52px Inter SemiBold -0.015em #CDD6F4, left-aligned, max 3 lines.

BELOW HEADLINE (y: 350):
Subhead in 20px Inter Regular #A6ADC8: "One-click workflow. Bookmarks and sessions sync to Warp's AI command bar."

TERMINAL MOCKUP (x: 60-1140, y: 410-580):
A "Warp-style" block — wider than tall, rounded 10px, fill #181825, 1px border #45475A. No traffic-light dots (Warp blocks don't have them). A small leading "block index" tag on the left: "[12]" in 12px JetBrains Mono #6C7086.

Inside, 18px padding, 15px JetBrains Mono, 24px line-height:
  Line 1: "$ claude-picker" in #CDD6F4, with a small "↑ runnable" tag on the right in 11px #74C7EC.
  Line 2 (output): "▸ ● auth-refactor     5m   45 msgs   $0.42" (standard color treatment).
  Line 3 (output): "  ● payment-gateway   2h   28 msgs   $0.18"
  Line 4 (output): "  ● k8s-deployment    1d   67 msgs   $1.05"
  Line 5 (output): "  ○ session           4h   12 msgs   $0.08"
  Line 6 (status): "→ 3 named · 1 recent · arrow keys to select" in 12px #6C7086.

To the right of the block, vertically centered at block-height, a small 32x32 Warp AI chip — a rounded square 8px radius, fill #1E1E2E, 1px border #74C7EC, containing the letter "W" in 18px Inter Bold #74C7EC. Above it: tiny 10px #6C7086 label: "ai".

Mood: collab, integration, quiet excitement, Warp-aesthetic-friendly.

NEGATIVE: no emoji, no Warp logo imitation (the W chip is stylized), no gradient, no 3D, no animated shimmer, no human figures.
```

---

## 25. Age-Warning Color Key — 1200x600 — Docs inline reference

**Where it's used:** Docs reference image showing how timestamps are color-coded by age.

**Safe zone:** Central **1060x520**.

**Background:** Flat `#1E1E2E`.

**Prompt:**

```
1200x600 reference card explaining age-coded timestamps. Flat #1E1E2E background. No halo.

Header (y: 50, centered): "Timestamp Color Key" in 26px Inter SemiBold #CDD6F4.
Sub (y: 90, centered): "Older sessions fade from fresh green to warning red." in 16px Inter Regular #A6ADC8.

MAIN GRID (y: 150-560): five rows, each 70px tall, spanning x=100 to x=1100. Each row shows:
  - A left color swatch 40x40 square, rounded 6px.
  - A center label: age range (Inter SemiBold 18px).
  - A right example: a mock session row in 15px JetBrains Mono showing the color in context.

Row 1:
  Swatch #A6E3A1. Label "< 1 hour — fresh". Example: "● auth-refactor                5m   45 msgs" with the "5m" in #A6E3A1 and rest standard.

Row 2:
  Swatch #F9E2AF. Label "1–6 hours — today". Example: "● payment-gateway              2h   28 msgs" with "2h" in #F9E2AF.

Row 3:
  Swatch #FAB387. Label "6–24 hours — recent". Example: "● drizzle-migration            18h   67 msgs" with "18h" in #FAB387.

Row 4:
  Swatch #F38BA8. Label "1–7 days — aging". Example: "● k8s-deployment               3d   52 msgs" with "3d" in #F38BA8.

Row 5:
  Swatch #6C7086. Label "> 7 days — stale". Example: "○ old-playground               2w   41 msgs" with everything in #6C7086.

Footer (y: 570, centered): "configurable via --age-colors flag" in 12px Inter Regular #585B70.

Mood: reference-card, didactic, quiet, docs-grade.

NEGATIVE: no emoji, no gradient between swatches, no 3D, no photograph, no decorative elements, the 5 colors must match the palette tokens exactly.
```

---

## GIF Recording Section

GIFs are captured from a real terminal (not AI-generated). They go in `assets/gifs/` and WebM versions go in `assets/videos/`. See `content/USAGE.md` and `scripts/tapes/README.md` for the recording workflow.

### Required GIFs

| File | Content | Dimensions | Length |
|---|---|---|---|
| `hero.gif` | Full flow: launch → project → session → preview → resume | 1400x800 | 12–15s |
| `search.gif` | `claude-picker --search authentication` → scroll results → pick | 1000x600 | 8–10s |
| `stats.gif` | `claude-picker --stats` → panels scroll into view → keypress | 1000x600 | 6–8s |
| `tree.gif` | `claude-picker --tree` → expand → navigate fork → pick | 1000x600 | 8–10s |
| `diff.gif` | `claude-picker --diff <a> <b>` → scroll diverging turns | 1400x800 | 10–12s |
| `bookmarks.gif` | Session picker → Ctrl+B pin → re-launch → pinned on top | 1000x600 | 8–10s |
| `export.gif` | Ctrl+E → toast → reveal `.md` file → preview in editor | 1000x600 | 6–8s |
| `thumb-hero.gif` | Trimmed first 5s of `hero.gif` for cards | 600x400 | 4–5s |
| `thumb-search.gif` | Trimmed 3s of `search.gif` | 600x400 | 3s |
| `thumb-stats.gif` | Trimmed 3s of `stats.gif` | 600x400 | 3s |

Each recording has a matching `.tape` file in `vhs/` (e.g., `vhs/hero.tape`, `vhs/search.tape`). Do not inline the tape scripts here — they live next to the GIFs they produce.

### Recording Tips

- **Terminal font size: 18–22 px minimum.** At smaller sizes, details are lost when embedded in blog posts.
- **Font:** JetBrains Mono at weight 400 for output, 500 for prompts. Fallback: Fira Code.
- **Theme:** Catppuccin Mocha terminal theme (match the design system exactly). Use `iTerm2-Color-Schemes` repo for the theme file.
- **Cursor:** Block cursor, blinking off during recording (distracting).
- **Recording tool of choice:** [VHS by Charm](https://github.com/charmbracelet/vhs). Deterministic, reproducible, version-controlled via `.tape` scripts.
- **Demo data:** Use a dedicated fixture directory `demo-data/` with curated session names. Do not record over real `~/.claude/` data — the names leak project secrets.
- **Sample session names for fixtures:** `auth-refactor`, `payment-gateway`, `k8s-deployment`, `drizzle-migration`, `logs-overhaul`, `feat-billing`, `portfolio-redesign`, `api-rate-limit`. Avoid client names, internal codenames, or anything unshippable.

### ffmpeg Optimization

Standard pipeline after VHS produces an uncompressed `.gif`:

```
# Scale + lossy-optimize with gifski (best quality/size ratio)
ffmpeg -i raw.gif -vf "fps=15,scale=1400:-1:flags=lanczos" -f yuv4mpegpipe - | \
  gifski -o hero.gif --quality 90 --fps 15 -

# Fallback (ffmpeg-only, slightly larger file)
ffmpeg -i raw.gif \
  -vf "fps=12,scale=1400:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128[p];[s1][p]paletteuse=dither=bayer:bayer_scale=5" \
  -loop 0 hero.gif
```

Target file sizes: hero ≤ 3.5 MB, feature GIFs ≤ 2 MB, thumbnail GIFs ≤ 800 KB. Above these, GitHub's feed viewer stalls.

### Suggested Dimensions (repeat for clarity)

- **Main demo GIF:** 1400x800 (maps to full-width README embed).
- **Feature GIFs:** 1000x600 (maps to half-width blog embed).
- **Thumbnail GIFs:** 600x400 (maps to Twitter inline preview).

---

## Image Generation Tools Section

Pick the tool that matches the image class. Generation quality varies significantly by tool and by image type.

### Primary: Gemini AI Pro (Imagen 4)

- **Strengths:** Photorealistic terminal mockups, accurate text rendering at 16–22 px, consistent palette adherence, honors hex codes when explicitly listed.
- **Use for:** Images 1, 2, 6, 7, 8, 9, 10, 11, 19, 21, 22 (anything with visible monospace text).
- **Tips:** Paste the full prompt verbatim. Request "no text artifacts, no garbled characters." Iterate twice and pick the cleaner of two.

### Fallback 1: OpenAI DALL-E 3 / GPT-Image

- **Strengths:** Clean editorial typography, strong at large display text, good at diagrammatic compositions.
- **Weaknesses:** Struggles with small monospace text (< 14 px), often garbles session-list columns, tends to "hallucinate" ligatures.
- **Use for:** Images 3, 4, 5, 13, 15, 17, 23, 25 (typographic-heavy, diagrammatic, or large-text-dominant).
- **Tips:** Explicitly say "monospace text must be legible and not warped." Use the prompt prefix "photorealistic design mockup."

### Fallback 2: Midjourney v6

- **Strengths:** Moody editorial aesthetic, atmospheric halos, best for the "Wired magazine" feel of Image 2.
- **Weaknesses:** Weakest text rendering. Cannot reliably produce exact hex codes without iteration.
- **Use for:** Image 2 (moody hero). Possibly Image 11 for the diff aesthetic but verify text integrity.
- **Tips:** Use `--style raw --s 50 --v 6 --ar 2:1` (or matching ratio). Append palette hex list at the end of the prompt.

### Alternative: Flux.1 Pro

- **Strengths:** Best-in-class text rendering for large text (> 18 px). Consistent with hex codes.
- **Weaknesses:** Weaker at atmosphere than Midjourney, weaker at complex UI mockups than Gemini.
- **Use for:** Images 5, 15, 16, 17, 20, 25 (large-text, logo, and reference cards).

### Non-AI: Terminal Screenshot Tools

For any image where authenticity trumps aesthetics — **record and screenshot a real terminal**. These tools work well:

- **[VHS (Charm)](https://github.com/charmbracelet/vhs):** Scripted terminal recording, deterministic output. Produces `.gif` and individual frames — screenshot the perfect frame and you have the image. Best for Images 6, 7, 8, 9, 10, 11.
- **[freeze (Charm)](https://github.com/charmbracelet/freeze):** Produces static images of code or terminal output with configurable padding, borders, themes. Ideal for Image 25 (reference card) and quick blog-inline snippets.
- **[carbon.now.sh](https://carbon.now.sh):** Code-screenshot SaaS. Good for code blocks but limited theming fidelity. Use for Image 13's markdown code block if needed.
- **[ray.so](https://ray.so):** Raycast's code-screenshot tool. Beautiful gradients but tricky to force into strict Catppuccin Mocha — adjust theme manually.

**Rule of thumb:** For the 10 images with dense monospace content (terminals, diffs, stats, trees), record a real terminal with VHS and screenshot a frame. The result is always better than AI-generated equivalents, and it matches what users will actually see.

---

## File Output Convention

All AI-generated images go under `assets/ai-generated/<platform>/` so they're grouped with the platform that consumes them. Saved paths for each of the 25 prompts in this file:

```
Prompt #  →  Save as

 1 → assets/ai-generated/github/social-preview.png       (1280x640)
 2 → assets/ai-generated/medium/hero.png                 (1400x788)
 3 → assets/ai-generated/medium/before-after.png         (1200x600)
 4 → assets/ai-generated/medium/architecture.png         (1200x800)
 5 → assets/ai-generated/linkedin/og-image.png           (1200x630)
 6 → assets/ai-generated/medium/project-picker.png       (1200x600)
 7 → assets/ai-generated/medium/session-preview.png      (1200x600)
 8 → assets/ai-generated/medium/feature-search.png       (1200x600)
 9 → assets/ai-generated/medium/feature-stats.png        (1200x800)
10 → assets/ai-generated/medium/feature-tree.png         (1200x800)
11 → assets/ai-generated/medium/feature-diff.png         (1400x800)
12 → assets/ai-generated/twitter/bookmarks-card.png      (1200x630)
13 → assets/ai-generated/twitter/export-card.png         (1200x630)
14 → assets/ai-generated/twitter/cost-card.png           (1200x675)
15 → assets/ai-generated/github/logo.png                 (512x512)
     assets/ai-generated/github/logo-transparent.png     (512x512, alpha)
16 → assets/ai-generated/github/favicon-16.png           (16x16)
     assets/ai-generated/github/favicon-32.png           (32x32)
     assets/ai-generated/github/favicon-64.png           (64x64)
     assets/ai-generated/github/apple-touch-180.png      (180x180)
17 → assets/ai-generated/twitter/card-generic.png        (1200x675)
18 → assets/ai-generated/reddit/thumbnail.png            (1200x630)
19 → assets/ai-generated/producthunt/gallery-01.png      (1270x760)
20 → assets/ai-generated/producthunt/thumbnail.png       (240x240)
21 → assets/ai-generated/instagram/story-template.png    (1080x1920)
22 → assets/ai-generated/linkedin/slide-template.png     (1080x1350)
23 → assets/ai-generated/twitter/skill-card.png          (1200x630)
24 → assets/ai-generated/twitter/warp-card.png           (1200x630)
25 → assets/ai-generated/medium/age-warnings.png         (1200x600)
```

**Story and carousel slides** (from `instagram-linkedin.md`) have their own naming pattern:

```
Instagram (10):  assets/ai-generated/instagram/story-01-hook.png … story-10-cta.png
LinkedIn (12):   assets/ai-generated/linkedin/slide-01-cover.png … slide-12-cta.png
```

Every image can optionally ship with a matching `.webp` (50–60% smaller) for web embeds. Keep the original PNG for editing; use the .webp on Medium and the GitHub README.

See `content/USAGE.md` for the complete platform-to-asset map (which image attaches to which tweet, which carousel slide, etc.).

---

Last updated: 2026-04-16. Palette version: Catppuccin Mocha. Logo mark: mauve lens + chevron, single-color vector. Total images: 25 plus logo variants and GIFs.

# Image Generation Prompts — Research-Backed Edition

Based on analysis of top repos (fzf 67k stars, lazygit 56k, bat 51k, starship 47k), Linear/Raycast/Warp design language, and AI prompt best practices.

**Color palette:** Catppuccin Mocha (the most popular dark theme for marketing)
- Background: #1E1E2E (base) / #181825 (mantle) / #11111B (crust)
- Text: #CDD6F4
- Accent purple: #CBA6F7 (mauve)
- Green: #A6E3A1
- Yellow: #F9E2AF
- Cyan/Blue: #89B4FA
- Pink: #F5C2E7
- Peach: #FAB387

---

## 1. GitHub Social Preview (1280x640) — MOST IMPORTANT

This appears when your repo link is shared ANYWHERE. 2x CTR boost over repos without one.

**Prompt:**

```
Product photograph style image of a developer tool interface. Dark background using color #1E1E2E (Catppuccin Mocha base). 

Left side, vertically centered: The word "claude-picker" in large bold monospace font (JetBrains Mono style), color #CDD6F4. Below it in smaller text, color #6C7086: "find, preview, and resume your Claude Code sessions"

Right side: A minimal terminal window mockup with rounded corners, background #181825, showing a styled list:
- Line 1: A small mauve/purple dot (#CBA6F7) followed by "auth-refactor" in green (#A6E3A1), "5m ago" in gray (#6C7086), "45 msgs" in gray
- Line 2: A small mauve dot followed by "fix-race-condition" in green, "2h ago" in gray, "28 msgs"  
- Line 3: A small mauve dot followed by "drizzle-migration" in green, "1d ago" in gray
- Line 4: A gray dot (#6C7086) followed by "session" in dim gray

The terminal window has a subtle purple glow around it — very subtle, 10% opacity of #CBA6F7 with large blur radius.

Dimensions: 1280x640 pixels. Keep core content within central 800x420 safe zone. No gradients, no flashy effects. Clean, minimal, dark. Inspired by Linear and Raycast design aesthetic. No emoji.
```

---

## 2. Medium Hero Image (1400x788)

First thing readers see in the Medium feed. Needs to be moody and editorial.

**Prompt:**

```
An atmospheric, cinematic-style image of a dark terminal interface floating in dark space. Background: deep black (#11111B) with an extremely subtle grid pattern (thin lines at 5% white opacity).

In the center: A terminal window with rounded corners, background #1E1E2E, showing a session picker interface with colored text:
- Yellow dots (#F9E2AF) next to green (#A6E3A1) session names: "auth-refactor", "payment-gateway", "k8s-deployment"
- Gray timestamps on the right
- A right-side panel showing conversation preview with "you:" in blue (#89B4FA) and "ai:" in yellow (#F9E2AF)

The terminal window has a soft purple/mauve rim glow (#CBA6F7 at 15% opacity, large blur radius) making it look like it's floating.

Above the terminal in clean white (#CDD6F4) sans-serif text: "I Reverse-Engineered Claude Code's Session Storage"
Below the terminal in smaller peach (#FAB387) text: "432 lines. bash + python + fzf."

Style: moody, editorial, premium. Like a Wired magazine cover for developers. Dark, atmospheric. Catppuccin Mocha color palette throughout. 1400x788 pixels.
```

---

## 3. Before vs After Comparison (1200x600) — for Medium inline

**Prompt:**

```
A side-by-side comparison image split vertically into two equal panels on a dark background (#11111B).

LEFT PANEL: 
Label at top: "before" in soft red (#F38BA8), small caps
A terminal window (background #1E1E2E) showing ugly, hard-to-read content:
  ? Pick a conversation to resume
  4a2e8f1c-9b3d-4e7a... (2 hours ago)
  b7c9d2e0-1f4a-8b6c... (3 hours ago)
  e5f8a3b1-7c2d-9e0f... (yesterday)
The text should look cluttered and confusing. No colors, all monochrome gray.

RIGHT PANEL:
Label at top: "after" in green (#A6E3A1), small caps
A terminal window (background #1E1E2E) showing a clean, colorful session picker:
  ● auth-refactor          5m ago    45 msgs
  ● fix-race-condition     2h ago    28 msgs  
  ● drizzle-migration      1d ago    67 msgs
With yellow dots (#F9E2AF), green names (#A6E3A1), and a preview panel on the right showing colored conversation text.

Thin vertical divider line between panels: #313244

Style: clean, high contrast between the two panels. The left should feel frustrating, the right should feel organized and premium. Catppuccin Mocha palette. 1200x600 pixels.
```

---

## 4. Architecture Diagram (1200x800) — for Medium + GitHub README

**Prompt:**

```
A clean, minimal system architecture diagram on a dark background (#1E1E2E).

The diagram shows this flow with connected boxes and thin arrow lines:

TOP ROW (two boxes side by side):
- Box 1: "~/.claude/projects/" with small label "JSONL session files" — border color #89B4FA
- Box 2: "~/.claude/sessions/" with small label "metadata" — border color #89B4FA

MIDDLE (one highlighted box):
- Box 3: "claude-picker" — highlighted with a mauve/purple border (#CBA6F7) and subtle glow. This is the main entry point.

BOTTOM ROW (two boxes):
- Box 4: "session-list.sh" with label "builds fzf list" — border color #A6E3A1
- Box 5: "session-preview.py" with label "renders preview" — border color #A6E3A1

BOTTOM CENTER:
- Box 6: "fzf" with label "interactive picker" — border color #F9E2AF

FINAL:
- Box 7: "claude --resume" with label "opens session" — border color #FAB387

Thin white arrow lines (#6C7086) connecting the boxes in flow order. All boxes have rounded corners, dark fill (#181825), and subtle borders. Labels in small gray text (#6C7086). Main text in white (#CDD6F4). Monospace font for file names.

Style: Excalidraw-like but polished. Catppuccin Mocha palette. No 3D effects. 1200x800 pixels.
```

---

## 5. Twitter Card (1200x675) — for tweets without GIF

**Prompt:**

```
A bold, high-contrast social media card on a dark background (#11111B) with a very subtle grid pattern (barely visible).

Center: Large white bold text (#CDD6F4): "claude-picker"
Below it: "Stop clicking through UUIDs." in peach (#FAB387), medium size.
Below that: "Browse, preview, and resume Claude Code sessions." in gray (#6C7086), smaller.

Bottom section: Three small pill-shaped badges in a row, each with dark fill (#313244) and subtle border (#45475A):
- "bash + python + fzf"
- "432 lines"  
- "any terminal"
Text inside badges in gray (#A6ADC8).

Top right corner: A small minimal terminal prompt icon in mauve (#CBA6F7).

Style: Bold, typographic, no terminal mockups. Think Linear or Vercel announcement card aesthetic. Clean sans-serif font. Catppuccin Mocha palette. 1200x675 pixels.
```

---

## 6. Reddit Post Image (1200x630) — must be readable as small thumbnail

**Prompt:**

```
A simple, high-contrast image on dark background (#1E1E2E). Must be readable when scaled to a small thumbnail.

Left side (40% width): A minimal terminal window with rounded corners, background #181825, showing 4 lines:
- Yellow dot + "auth-refactor" in green + "5m ago" in gray
- Yellow dot + "payment-gateway" in green + "2h ago"
- Yellow dot + "k8s-deployment" in green + "1d ago"
- Gray dot + "session" in dim gray + "4h ago"

Right side (60% width): Large white bold text (#CDD6F4) vertically centered:
"claude-picker"
Below in smaller gray (#6C7086): "Session manager for Claude Code"
Below that, even smaller: "github.com/anshul-garg27/claude-picker" in mauve (#CBA6F7)

Style: Ultra-simple. Maximum contrast. No gradients, no glows, no effects. Must be clearly readable at 70x70 pixel thumbnail size. Dark background, bright foreground. 1200x630 pixels.
```

---

## 7. Logo / Icon (512x512) — for GitHub avatar, social profiles

**Prompt:**

```
A minimal, geometric icon/logo on a dark background (#1E1E2E).

The icon combines two visual concepts:
1. A magnifying glass (representing search/find) 
2. A terminal cursor bracket ">" (representing CLI)

The magnifying glass handle forms the ">" bracket shape. Single color: mauve/purple (#CBA6F7).

The icon should be geometric, flat, and recognizable at 16x16 pixels. No text. No gradients. Just the mauve icon shape on the dark background.

Square format, 512x512 pixels. Think of how Raycast or Linear do their icons — simple, geometric, instantly recognizable.
```

---

## 8. Open Graph / Link Sharing Image (1200x630)

When someone shares your article/repo on LinkedIn, Slack, Discord.

**Prompt:**

```
A typographic social sharing image on dark background (#11111B) with an extremely subtle grid pattern (#ffffff at 3% opacity).

Large white bold text (#CDD6F4) centered: 
"I Reverse-Engineered How Claude Code Stores Sessions"

Below in peach (#FAB387), slightly smaller:
"and built a 432-line tool to browse them"

Bottom left: "by Anshul Garg" in gray (#6C7086)
Bottom right: Small GitHub icon (gray) + "claude-picker" in mauve (#CBA6F7)

Style: editorial, confident, typographic. Like a conference talk title card. No terminal screenshots, no mockups — pure typography on dark. Clean sans-serif font. Keep text within central safe zone. 1200x630 pixels.
```

---

## 9. Medium Inline: Project Picker Screenshot (1200x600) — NEW

Show the first step of the tool — the project directory picker.

**Prompt:**

```
A terminal screenshot mockup on dark background (#181825) with rounded window corners and a subtle window title bar.

Inside the terminal, show a styled project picker list:
Header text in mauve (#CBA6F7): "claude-picker"

Four rows:
1. "architex" in bold cyan (#89B4FA), "just now" in gray, green bar "█████" (#A6E3A1), "5 sessions" in gray
2. "ecommerce-api" in bold cyan, "2m ago" in gray, green bar "███", "3 sessions" in gray
3. "infra-automation" in bold cyan, "1h ago" in gray, green bar "██", "2 sessions"
4. "portfolio-site" in bold cyan, "3h ago" in gray, green bar "██", "2 sessions"

The first row has a mauve pointer arrow "▸" on the left indicating it's selected.

At the bottom: "project >" prompt text in cyan.

Style: Realistic terminal look. Catppuccin Mocha colors. Monospace font. Dark, clean. 1200x600 pixels.
```

---

## 10. Medium Inline: Session Picker with Preview (1200x600) — NEW

Show the second step — session list with the conversation preview panel.

**Prompt:**

```
A terminal screenshot mockup split into two sections (60/40 split).

LEFT SECTION (the session list):
Header: "architex" in mauve (#CBA6F7) with "enter open | ctrl-d delete" in dim gray
A section label: "── saved ──" in dim gray (#6C7086)
Rows:
- Mauve pointer "▸" + yellow dot (#F9E2AF) + "auth-refactor" in bold green (#A6E3A1) + "5m ago" + "45 msgs"
- Yellow dot + "fix-race-condition" in green + "2h ago" + "28 msgs"
- Yellow dot + "drizzle-migration" in green + "1d ago" + "67 msgs"
Section label: "── recent ──" in dim gray
- Gray dot + "session" in dim gray + "4h ago" + "12 msgs"

Prompt: "session >" in cyan at bottom.

RIGHT SECTION (preview panel, separated by a thin vertical line):
Header: "auth-refactor" in bold green
"created  2026-04-16 14:30" in gray
"messages 45" in gray
Thin horizontal line
Conversation:
"you" in bold cyan + "the auth middleware is storing session tokens..."
"ai" in yellow + "I'll restructure the session token storage to use encrypted..."
"you" in bold cyan + "also need to handle the refresh token flow..."

All on dark background (#1E1E2E). Monospace font. Catppuccin Mocha. 1200x600 pixels.
```

---

## 11. Twitter Thread: Feature Highlight Cards (1200x675 each) — NEW

One image per tweet. Clean, typographic, one feature per card.

**Prompt for Card 1 (Search):**
```
Dark background (#11111B). Large emoji-free icon of a magnifying glass in mauve (#CBA6F7) on the left. Right side: Bold white text "Fuzzy Search" and below in gray "Type to filter sessions instantly. Find any conversation in seconds." Catppuccin Mocha. 1200x675.
```

**Prompt for Card 2 (Preview):**
```
Dark background (#11111B). Large icon of an eye/preview symbol in cyan (#89B4FA) on the left. Right side: Bold white text "Conversation Preview" and below in gray "See the last few messages before opening. No more guessing." Catppuccin Mocha. 1200x675.
```

**Prompt for Card 3 (Delete):**
```
Dark background (#11111B). Large icon of a trash/X symbol in soft red (#F38BA8) on the left. Right side: Bold white text "Ctrl+D to Delete" and below in gray "Clean up old sessions without leaving the picker." Catppuccin Mocha. 1200x675.
```

**Prompt for Card 4 (Named Sessions):**
```
Dark background (#11111B). Large star icon in yellow (#F9E2AF) on the left. Right side: Bold white text "Named Sessions First" and below in gray "Sessions created with --name appear on top. Always find what matters." Catppuccin Mocha. 1200x675.
```

---

## Summary: Complete Image Inventory

| # | Image | Size | For |
|---|-------|------|-----|
| 1 | GitHub Social Preview | 1280x640 | Repo settings → Social preview |
| 2 | Medium Hero | 1400x788 | Top of article |
| 3 | Before vs After | 1200x600 | Medium inline (the problem) |
| 4 | Architecture Diagram | 1200x800 | Medium + GitHub README |
| 5 | Twitter Card | 1200x675 | Tweet 1 (if no GIF) |
| 6 | Reddit Thumbnail | 1200x630 | Reddit posts |
| 7 | Logo/Icon | 512x512 | GitHub avatar, social profiles |
| 8 | Open Graph | 1200x630 | Link sharing (LinkedIn, Slack) |
| 9 | Project Picker Screenshot | 1200x600 | Medium inline (step 1) |
| 10 | Session Picker + Preview | 1200x600 | Medium inline (step 2) |
| 11a | Feature Card: Search | 1200x675 | Twitter thread tweet 2 |
| 11b | Feature Card: Preview | 1200x675 | Twitter thread tweet 3 |
| 11c | Feature Card: Delete | 1200x675 | Twitter thread tweet 4 |
| 11d | Feature Card: Named | 1200x675 | Twitter thread tweet 5 |

**Total: 14 images** to generate in Gemini AI Pro.

## GIF Recording (do separately)

Not an AI-generated image — this needs real terminal recording:

1. Make terminal font BIG (`Cmd + +` several times)
2. `Cmd + Shift + 5` → Record Selected Portion → select terminal only
3. Run `claude-picker`, navigate both steps, show preview, select a session
4. Keep under 15 seconds
5. Convert: `ffmpeg -i recording.mov -vf "fps=12,scale=800:-1:flags=lanczos" -loop 0 demo.gif`

The GIF goes in: GitHub README (hero), Twitter tweet 1, Reddit posts.

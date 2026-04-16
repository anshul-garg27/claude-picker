# Image Generation Prompts (Gemini AI Pro)

Each platform needs different visuals. Here are ready-to-paste prompts.

---

## 1. GitHub Social Preview (1280x640)

This appears when someone shares your repo link on any platform. Most important image.

**Prompt:**

```
Design a minimal, dark-themed social preview banner for a developer tool called "claude-picker". 

Dimensions: 1280x640 pixels.

Background: deep dark gradient (#0d1117 to #161b22), similar to GitHub's dark mode.

Left side: The text "claude-picker" in a modern monospace font (like JetBrains Mono), large, white bold. Below it in smaller gray text: "Find, preview, and resume your Claude Code sessions"

Right side: A stylized minimal terminal window mockup showing a fzf-style list with these items:
- A magenta "+" icon with "New Session" in cyan
- A yellow dot with "auth-refactor" in green  
- A yellow dot with "fix-bug-123" in green
- A gray dot with "session" in dim gray
The terminal should have a dark background with rounded corners.

Bottom right corner: small "github.com/anshul-garg27" in dim gray text.

Style: clean, minimal, developer-focused. No gradients or flashy effects. Think Vercel or Raycast aesthetic. No emojis.
```

---

## 2. Medium Hero Image (1400x788)

First thing readers see. Needs to be eye-catching in the Medium feed.

**Prompt:**

```
Create a hero image for a technical blog post about a terminal-based developer tool.

Dimensions: 1400x788 pixels.

The image shows a dark terminal screen (black/dark gray background) with a glowing, slightly blurred terminal window in the center. Inside the terminal, show a stylized session picker interface with:
- Colored dots (yellow, green) next to session names like "auth-refactor", "fix-bug-123", "drizzle-migration"
- Relative timestamps like "5m ago", "2h ago"  
- A right panel showing a conversation preview with "you:" in cyan and "ai:" in yellow

The terminal window should have a subtle purple/magenta glow around it, like it's floating in dark space. Very subtle, not overdone.

Above the terminal, in clean sans-serif white text: "I Reverse-Engineered Claude Code's Session Storage"
Below: "432 lines. bash + python + fzf." in smaller gray text.

Style: moody, editorial, technical. Like a Wired magazine cover for developers. Dark, atmospheric, premium feel.
```

---

## 3. Medium Inline — "Before vs After" (1200x600)

For inside the article, showing the pain point vs the solution.

**Prompt:**

```
Create a side-by-side comparison image for a developer blog post. Split into two panels.

Dimensions: 1200x600 pixels. Dark background.

LEFT PANEL (labeled "Before" in red/orange text at top):
A terminal showing a plain, ugly list of session UUIDs:
  4a2e8f1c-9b3d-4e7a... (2 hours ago)
  b7c9d2e0-1f4a-8b6c... (3 hours ago)
  e5f8a3b1-7c2d-9e0f... (yesterday)
A confused face emoji or a red X mark. Feels frustrating and messy.

RIGHT PANEL (labeled "After" in green text at top):
A polished terminal showing a clean session picker with:
  ● auth-refactor       5m ago   45 msgs
  ● fix-bug-123         2h ago   12 msgs
  ○ session             1d ago    6 msgs
With a preview panel on the right showing conversation snippets.
A green checkmark. Feels clean and organized.

Dividing line between panels: thin white or gray vertical line.

Style: clean, minimal, dark theme. The contrast between messy left and polished right should be immediately obvious.
```

---

## 4. Twitter Card Image (1600x900)

For tweets that don't have a GIF — a static image that grabs attention in the feed.

**Prompt:**

```
Design a bold, attention-grabbing social media card for a developer tool.

Dimensions: 1600x900 pixels.

Dark background (#0f0f0f). 

Center: Large white bold text: "claude-picker"
Below it: "Stop clicking through UUIDs." in a slightly smaller, orange/amber color.
Below that: "Browse, preview, and resume Claude Code sessions." in gray.

Bottom section: Three small icons/badges in a row:
- "bash + python + fzf" 
- "432 lines"
- "any terminal"
Each in a subtle rounded dark pill/badge shape with dim borders.

Top right corner: A small terminal icon or command prompt icon.

Style: bold, high contrast, dark mode. Think product launch card. Clean sans-serif font. No images of terminals — just typography. Similar to how Linear or Vercel do their announcement cards.
```

---

## 5. Reddit Post Image (1200x630)

Reddit thumbnails are small. Needs to be readable at thumbnail size.

**Prompt:**

```
Create a simple, high-contrast image for a Reddit post about a CLI developer tool.

Dimensions: 1200x630 pixels.

Dark background. 

Left side: A minimal terminal window with rounded corners showing 4-5 lines of a session picker:
  ● auth-refactor      5m ago
  ● fix-bug-123        2h ago
  ● migration           1d ago
  ○ session             3h ago
Use yellow dots for named, gray for unnamed. Green text for names.

Right side: Large white text vertically centered:
"claude-picker"
Below in small gray: "Session manager for Claude Code"

Style: ultra-simple, must be readable as a small thumbnail. High contrast. No gradients, no glows, no effects. Just terminal + text. Dark background, white/green/yellow foreground.
```

---

## 6. Architecture Diagram (for Medium article) (1200x800)

Shows how the tool works internally — adds technical credibility.

**Prompt:**

```
Create a clean, minimal architecture diagram for a CLI tool.

Dimensions: 1200x800 pixels. Dark background (#1a1a2e).

Show this flow with connected boxes and arrows:

Box 1 (top left): "~/.claude/projects/" with label "JSONL session files"
Box 2 (top right): "~/.claude/sessions/" with label "Session metadata"

Arrow from both boxes pointing down to:

Box 3 (center): "claude-picker" (main script) — highlighted with a subtle magenta border

Arrow from Box 3 going down-left to:
Box 4: "session-list.sh" with label "Builds fzf list"

Arrow from Box 3 going down-right to:
Box 5: "session-preview.py" with label "Renders preview"

Arrow from Box 4 and Box 5 going down to:
Box 6 (bottom center): "fzf" with label "Interactive picker"

Arrow from Box 6 to:
Box 7 (bottom): "claude --resume <id>" with label "Opens session"

Style: clean, minimal, use thin white lines for arrows. Boxes should have rounded corners, subtle borders, dark fill. Labels in small gray text. Main text in white. Use monospace font for file names. Think Excalidraw or Mermaid diagram aesthetic but polished.
```

---

## 7. Logo / Icon (512x512)

For GitHub avatar, social profiles, favicon.

**Prompt:**

```
Design a minimal, modern icon/logo for a developer tool called "claude-picker".

Dimensions: 512x512 pixels. Square, suitable for GitHub and social profile pictures.

The icon should combine two concepts:
1. A magnifying glass or search icon (representing "picking/finding")
2. A terminal/command prompt bracket (representing CLI tool)

Color: Use a soft magenta/purple (#b48ead or #c792ea) as the primary color on a dark background (#1e1e2e).

Style: geometric, minimal, single-color on dark background. No text in the icon. No gradients. Flat design. Think of how Raycast, Linear, or Arc browser do their icons — simple, geometric, recognizable at 16x16.
```

---

## 8. Open Graph Image for Blog (1200x630)

When someone shares your Medium article link on LinkedIn, Slack, Discord — this is what shows up.

**Prompt:**

```
Create an Open Graph social sharing image for a blog article.

Dimensions: 1200x630 pixels.

Dark background with very subtle grid pattern (like graph paper, barely visible).

Large white bold text centered: 
"I Reverse-Engineered How Claude Code Stores Sessions"

Below in smaller amber/orange text:
"and built a 432-line tool to browse them"

Bottom left: Small text "by Anshul Garg" in gray.
Bottom right: Small GitHub icon + "anshul-garg27/claude-picker" in gray.

Style: editorial, clean, confident. Like a conference talk title card. Dark background, high contrast text. No terminal screenshots or mockups — pure typography.
```

---

## Summary: Which Images Go Where

| Platform | Images Needed |
|----------|--------------|
| **GitHub README** | Demo GIF + Architecture diagram |
| **GitHub repo settings** | Social preview (1280x640) |
| **Medium article** | Hero image + Before/After + Architecture diagram |
| **Twitter** | Demo GIF (tweet 1) + Twitter card (for quote tweets) |
| **Reddit** | Reddit post image + Demo GIF |
| **Hacker News** | None (text only) — but OG image matters when shared |
| **LinkedIn/Slack/Discord shares** | Open Graph image (auto-pulled) |

## GIF Recording Instructions

**Option A: Screen record (recommended)**
1. Open a terminal, make the font size bigger (Cmd+Plus a few times)
2. Press `Cmd+Shift+5` on Mac → select "Record Selected Portion"
3. Select just the terminal window
4. Run `claude-picker`, navigate the picker, select a session
5. Stop recording (saves as .mov on Desktop)
6. Convert:
```bash
ffmpeg -i ~/Desktop/"Screen Recording"*.mov -vf "fps=12,scale=800:-1:flags=lanczos" -loop 0 ~/Desktop/claude-picker/demo.gif
```

**Option B: VHS (scripted)**
```bash
cd ~/Desktop/claude-picker && vhs demo.tape
```
This uses the `demo.tape` file I created. You may need to adjust the key timings.

**Tips for a good GIF:**
- Keep it under 15 seconds
- Make terminal font bigger (so it's readable on mobile)
- Show the FULL flow: project picker → session picker → preview → open
- Dark theme looks best in GIFs

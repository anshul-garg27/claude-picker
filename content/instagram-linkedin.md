# claude-picker: Social Media Launch Content
## Complete Content Kit for Instagram Stories + LinkedIn

> GitHub: github.com/anshul-garg27/claude-picker
> Generated: April 2026

---

# TABLE OF CONTENTS

1. [Research Summary & Strategy](#research-summary--strategy)
2. [Instagram Stories Content (10 Slides)](#instagram-stories-content)
3. [LinkedIn Text Post](#linkedin-text-post)
4. [LinkedIn Carousel (12 Slides)](#linkedin-carousel-12-slides)
5. [Hashtag Strategy](#hashtag-strategy)
6. [Posting Schedule](#posting-schedule)
7. [Image Generation Prompts (Gemini AI Pro)](#image-generation-prompts)

---

# RESEARCH SUMMARY & STRATEGY

## Platform Analysis

### Instagram for Developers — Does It Work?

Short answer: yes, but with caveats. Tech content is a growing niche on Instagram (over 60% of creator income comes from brand collaborations in tech, lifestyle, and similar niches). Instagram's 2026 "Year of Raw Content" shift means the algorithm now favors authenticity over polish — which benefits a solo developer sharing real tools over a corporate marketing team. That said, Instagram is a secondary channel for devtools. Use it for visual appeal, brand awareness, and reaching developers who cross-browse (many senior devs in their 30s-40s use Instagram daily). Stories are ideal for quick, punchy content that disappears — low commitment, high curiosity.

### LinkedIn for Developer Tool Launches — The Primary Channel

LinkedIn carousels generate 3.5x more engagement than text-only posts (6.60% avg engagement rate vs ~2% for text). PDF carousels have the longest lifespan of any LinkedIn format: 1-2 weeks of active reach. Over 67% of LinkedIn usage is mobile, and carousels feel native to touch-swipe behavior. The first 60-90 minutes determine 70% of your post's total reach — hook quality and timing are critical. The algorithm in 2026 measures "dwell time" — how long someone spends on your content. A 12-slide carousel that someone fully swipes is an enormous engagement signal.

## Competitive Positioning

| Feature | claude-picker | claude-history (Rust) | Claude Squad (Go) |
|---------|--------------|----------------------|-------------------|
| Per-session cost display | YES | No | No |
| Full-text search across all projects | YES | Partial | No |
| Stats dashboard (`--stats`) | YES | No | No |
| Session tree with forks (`--tree`) | YES | No | No |
| Session diff (`--diff`) | YES | No | No |
| Bookmarks (`Ctrl+B`) | YES | No | No |
| Markdown export (`Ctrl+E`) | YES | No | No |
| In-place delete (`Ctrl+D`) | YES | No | No |
| Two-step picker (project then session) | YES | No | No |
| Auto-named sessions from first message | YES | No | No |
| Age warnings (7d peach, 30d red) | YES | No | No |
| Git branch display per project | YES | No | No |
| Warp `+` menu integration | YES | No | No |
| `/claude-picker` Claude Code slash command | YES | No | No |
| Weight | ~800 lines bash+python | Compiled Rust | Compiled Go |
| Dependencies | bash, python, fzf, rich | Rust binary | Go binary |

**Our angle in all content**: Most feature-rich session manager for Claude Code. Unix philosophy. Only tool showing cost per session + tree + diff + search + stats together.

---

# INSTAGRAM STORIES CONTENT

**Format**: 1080 x 1920 px (9:16 ratio)
**Safe zone**: Keep critical content within 1080 x 1390 px (centered), avoiding top 250px and bottom 280px
**Color palette**: Catppuccin Mocha throughout

## Story Slide 1 of 10 — The Hook

**Text on screen:**
```
You have 47 Claude Code sessions.

Which one had that auth fix?

Good luck finding it.
```

**Visual direction:** Dark background (#1E1E2E). Text centered. "47" in large yellow (#F9E2AF). "Good luck finding it." in smaller muted gray (#6C7086), slightly sardonic. No images — just text on dark. Minimalist. The emptiness IS the point.

**Swipe-up text:** "Swipe to see the fix -->"

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Content fits within safe zone (avoid top 250px and bottom 280px), center-aligned with 80px horizontal padding. Three lines of text stacked with 48px line spacing:

Line 1: "You have 47 Claude Code sessions." in color #CDD6F4, clean sans-serif, medium weight, 56px. The number "47" is color #F9E2AF (golden yellow), bolder, 72px.
Line 2: "Which one had that auth fix?" in color #CDD6F4, 52px.
Line 3: "Good luck finding it." in color #6C7086 (muted gray), 44px, slightly italic.

No images, decorations, or gradients. Monospace or clean sans-serif typeface. Developer aesthetic. No emojis.
```

---

## Story Slide 2 of 10 — The Problem

**Text on screen:**
```
Claude Code doesn't have
a session manager.

Your conversations pile up.
Unnamed. Unsearchable. Forgotten.

You re-explain context.
You burn tokens.
You waste money.
```

**Visual direction:** Same dark background (#1E1E2E). Text in #CDD6F4. "burn tokens" in peach (#FAB387). "waste money" in red-ish peach. Left-aligned text. Each line stacked with breathing room. Terminal-style monospace font feel.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Left-aligned text in safe zone with 80px left padding, vertically centered. Monospace typography throughout, 40px body size.

"Claude Code doesn't have" in #CDD6F4
"a session manager." in #CDD6F4
[40px gap]
"Your conversations pile up." in #CDD6F4
"Unnamed. Unsearchable. Forgotten." in #6C7086
[40px gap]
"You re-explain context." in #CDD6F4
"You burn tokens." in #FAB387 (peach), bold
"You waste money." in #FAB387 (peach), bold

No decorations, images, or emojis. Terminal-aesthetic typography. Dark and stark.
```

---

## Story Slide 3 of 10 — Introducing claude-picker

**Text on screen:**
```
claude-picker

Browse. Preview. Resume.
Your Claude Code sessions.

fzf + Rich + python.
~800 lines. MIT.
```

**Visual direction:** Center-aligned. "claude-picker" in large purple (#CBA6F7) as hero text. "Browse. Preview. Resume." in green (#A6E3A1). The "~800 lines" stat in yellow (#F9E2AF). Below, a subtle terminal window outline (rounded rectangle in #313244) with fake blinking cursor. Background #1E1E2E.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Center-aligned layout in safe zone.

Top: "claude-picker" hero text in #CBA6F7 (purple), bold monospace, 96px.
Below (24px gap): "Browse. Preview. Resume." in #A6E3A1 (green), 44px. "Your Claude Code sessions." in #CDD6F4, 40px.
[60px gap]
Terminal window mockup centered: rounded rectangle, 760px wide, 180px tall, background #313244, border 1px #45475A. Title bar dots #F38BA8, #F9E2AF, #A6E3A1. Inside: "$ claude-picker" in #CDD6F4 monospace 32px, cursor block #CBA6F7.
[40px gap]
"fzf + Rich + python." in #F9E2AF (yellow), 32px.
"~800 lines. MIT." in #6C7086 (muted gray), 28px.

No photographs, gradients, or emojis. Clean developer aesthetic.
```

---

## Story Slide 4 of 10 — The Killer Feature (Cost)

**Text on screen:**
```
No other tool shows you this:

  auth-refactor      $0.47  12.3k tokens
  fix-race-condition $1.23  31.2k tokens
  drizzle-migration  $0.08   2.1k tokens

Per-session cost tracking.
Know exactly what you're spending.
```

**Visual direction:** This is the money slide (literally). Background #1E1E2E. Three session lines styled like terminal output, monospace font, inside a terminal window frame (#313244 border). Dollar amounts in green (#A6E3A1). Token counts in blue (#89B4FA). Session names in #CDD6F4. "No other tool shows you this" in yellow (#F9E2AF) at top. "Per-session cost tracking." bold, in peach (#FAB387). Feels like a real terminal.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Layout centered in safe zone.

Top: "No other tool shows you this:" in #F9E2AF (yellow), bold, 44px, center-aligned.
[40px gap]
Terminal window mockup: rounded rectangle 900x380px, background #313244, border #45475A. Title bar dots #F38BA8, #F9E2AF, #A6E3A1. Inside, three rows monospace 32px, left-aligned with column alignment:

Row 1: "auth-refactor" in #CDD6F4 | "$0.47" in #A6E3A1 | "12.3k tokens" in #89B4FA
Row 2: "fix-race-condition" in #CDD6F4 | "$1.23" in #A6E3A1 | "31.2k tokens" in #89B4FA
Row 3: "drizzle-migration" in #CDD6F4 | "$0.08" in #A6E3A1 | "2.1k tokens" in #89B4FA

Dollar column visually prominent.
[40px gap]
"Per-session cost tracking." in #FAB387 (peach), bold, 48px.
"Know exactly what you're spending." in #CDD6F4, 36px.

Authentic terminal feel. No emojis, no decorations beyond terminal frame.
```

---

## Story Slide 5 of 10 — How It Works

**Text on screen:**
```
Step 1: Pick your project
Step 2: Pick your session
Step 3: You're back in context

Live preview panel.
Fuzzy search via fzf 0.58+.
Rich-formatted conversation.
```

**Visual direction:** Three steps stacked vertically with step numbers in purple circles (#CBA6F7). Each step text in #CDD6F4. Below, three feature bullets — green (#A6E3A1) checkmarks before each. Background #1E1E2E. Clean, scannable.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Center-aligned layout in safe zone with 60px between steps.

Step 1: 80px circle with #CBA6F7 (purple) fill, "1" in #1E1E2E centered. Next to it: "Pick your project" in #CDD6F4, 42px monospace.
Step 2: Same circle with "2". Next to it: "Pick your session" in #CDD6F4, 42px.
Step 3: Same circle with "3". Next to it: "You're back in context" in #A6E3A1 (green), 42px bold — emphasize the payoff.

[60px gap]
Divider line 1px in #313244 spanning 600px centered.
[40px gap]
Three feature bullets, left-aligned with 40px left padding:
- Green checkmark #A6E3A1 + "Live preview panel" in #CDD6F4, 36px
- Green checkmark #A6E3A1 + "Fuzzy search via fzf 0.58+" in #CDD6F4, 36px
- Green checkmark #A6E3A1 + "Rich-formatted conversation" in #CDD6F4, 36px

No images, gradients, or emojis.
```

---

## Story Slide 6 of 10 — The Stats Dashboard

**Text on screen:**
```
claude-picker --stats

  Total sessions:     142
  Total tokens:    2.4M
  Total cost:    $38.27

  architex        ████████ $12.40
  design-system   ██████   $8.92
  infra           ████     $5.13

  today     ██ 8
  week      ████████ 34
  older     ████████████████ 100
```

**Visual direction:** Pure terminal aesthetic. Background #1E1E2E. Terminal frame (#313244). The three totals in #CDD6F4 with values colored (sessions blue, tokens yellow, cost green). Bar charts using block characters in #CBA6F7. Monospace everything.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Center-aligned layout in safe zone.

Top: "claude-picker --stats" in #CDD6F4 monospace, 40px, with "--stats" in #F9E2AF. 
[40px gap]
Large terminal window mockup 960x1100px, background #313244, border #45475A, three title bar dots.

Inside terminal, monospace 30px throughout, left-aligned 40px padding:

Section 1 — Totals:
"Total sessions:" #CDD6F4 + "142" in #89B4FA (blue)
"Total tokens:" #CDD6F4 + "2.4M" in #F9E2AF (yellow)
"Total cost:" #CDD6F4 + "$38.27" in #A6E3A1 (green)
[30px gap]

Section 2 — Per-project bars:
"architex       " #CDD6F4 + "████████" in #CBA6F7 + "$12.40" #A6E3A1
"design-system  " #CDD6F4 + "██████" in #CBA6F7 + "$8.92" #A6E3A1
"infra          " #CDD6F4 + "████" in #CBA6F7 + "$5.13" #A6E3A1
[30px gap]

Section 3 — Activity timeline:
"today    " #CDD6F4 + "██" in #A6E3A1 + "8" #CDD6F4
"week     " #CDD6F4 + "████████" in #F9E2AF + "34" #CDD6F4
"older    " #CDD6F4 + "████████████████" in #6C7086 + "100" #CDD6F4

[40px gap below terminal]
"Full dashboard. One command." in #CBA6F7 (purple), 40px, center-aligned.

No emojis. Pure terminal aesthetic. Block characters must render cleanly.
```

---

## Story Slide 7 of 10 — Session Tree with Forks

**Text on screen:**
```
claude-picker --tree

architex/
  ├─ auth-refactor
  │   └─ auth-refactor-retry (fork)
  │       └─ auth-refactor-v3 (fork)
  ├─ drizzle-migration
  └─ fix-race-condition

Every fork. Every branch. Visualized.
```

**Visual direction:** Tree diagram rendered in terminal style. Project name in purple. Tree characters in muted gray. Fork labels in peach to highlight relationship. Background #1E1E2E.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Center-aligned layout in safe zone.

Top: "claude-picker --tree" in #CDD6F4 monospace 40px, "--tree" in #F9E2AF.
[50px gap]
Terminal window 960x720px, background #313244, border #45475A, three title bar dots.

Inside terminal, monospace 32px, left-aligned 40px padding:

"architex/" in #CBA6F7 (purple), bold
"  ├─ auth-refactor" — tree characters in #6C7086, name in #CDD6F4
"  │   └─ auth-refactor-retry" — tree in #6C7086, name in #CDD6F4, " (fork)" in #FAB387 (peach)
"  │       └─ auth-refactor-v3" — tree in #6C7086, name in #CDD6F4, " (fork)" in #FAB387
"  ├─ drizzle-migration" — tree in #6C7086, name in #CDD6F4
"  └─ fix-race-condition" — tree in #6C7086, name in #CDD6F4

[40px gap below terminal]
"Every fork. Every branch. Visualized." in #A6E3A1 (green), 38px center-aligned.

Box-drawing characters must render cleanly. No emojis.
```

---

## Story Slide 8 of 10 — Full-Text Search

**Text on screen:**
```
claude-picker --search "race condition"

architex   fix-race-condition
  > "race condition in the websocket
    reconnect logic..."

infra      deploy-debug
  > "might be a race condition when
    both pods start at the same time"

Every message. Every session.
Every project. One command.
```

**Visual direction:** Search results in terminal frame. The match snippets in italic-feel #CDD6F4 on slightly darker background. Project and session names in blue/purple. Quoted highlight ">".

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Safe zone centered.

Top: "claude-picker --search \"race condition\"" in #CDD6F4 monospace 32px. "--search" in #F9E2AF, quoted query in #A6E3A1.
[40px gap]
Terminal window 960x900px, background #313244, border #45475A, three title bar dots.

Inside terminal, monospace 28px, left-aligned 32px padding:

Result 1:
"architex   fix-race-condition" — "architex" in #89B4FA (blue), session name in #CBA6F7 (purple).
"  > \"race condition in the websocket" in #CDD6F4
"    reconnect logic...\"" in #CDD6F4
[20px gap]

Result 2:
"infra      deploy-debug" — "infra" in #89B4FA, session in #CBA6F7.
"  > \"might be a race condition when" in #CDD6F4
"    both pods start at the same time\"" in #CDD6F4

[40px gap below terminal]
"Every message. Every session." in #F9E2AF (yellow), 36px center.
"Every project. One command." in #F9E2AF, 36px center.

Quote character ">" in #A6E3A1 (green), prominent.
No emojis.
```

---

## Story Slide 9 of 10 — In-Picker Shortcuts

**Text on screen:**
```
Power keys:

  Ctrl+B   bookmark (top of list)
  Ctrl+E   export to markdown
  Ctrl+D   delete in-place
  Ctrl+P   launch from anywhere

No config. No plugins. Just works.
```

**Visual direction:** Keys presented as rounded rectangles with purple fill, white key letters. Action descriptions in #CDD6F4. Tight, clean keycap aesthetic.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Center-aligned layout in safe zone.

Top: "Power keys:" in #CBA6F7 (purple), bold, 56px, center-aligned.
[60px gap]
Four rows, 50px vertical spacing, left-aligned with 120px left padding:

Row 1: Keycap group "[Ctrl]" + "[B]" — each keycap is rounded rectangle 120x60px, background #313244, border 1.5px #89B4FA (blue), letters #CDD6F4 monospace 32px. Gap between pair: 12px. Then 40px gap, then "bookmark (top of list)" in #CDD6F4 36px.

Row 2: "[Ctrl]" + "[E]" keycaps (same style). Then "export to markdown" in #CDD6F4.

Row 3: "[Ctrl]" + "[D]" keycaps. Then "delete in-place" in #CDD6F4.

Row 4: "[Ctrl]" + "[P]" keycaps. Then "launch from anywhere" in #CDD6F4.

[60px gap]
Bottom: "No config. No plugins. Just works." in #A6E3A1 (green), 40px, center-aligned.

Keycaps must look like real keyboard keys. No emojis.
```

---

## Story Slide 10 of 10 — CTA

**Text on screen:**
```
MIT licensed. Works now.

github.com/anshul-garg27/claude-picker

Star it. Try it. Tell me what breaks.
```

**Visual direction:** GitHub logo (simplified, white outline) at top. URL in blue (#89B4FA), styled to look clickable. "Star it. Try it. Tell me what breaks." in yellow (#F9E2AF) — casual, honest. Background #1E1E2E. Link sticker points to repo.

**Instagram link sticker:** github.com/anshul-garg27/claude-picker

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Center-aligned in safe zone.

Top: Simplified GitHub octocat silhouette as 120x120px outline in #CDD6F4, centered.
[40px gap]
"MIT licensed. Works now." in #CDD6F4, 42px, center-aligned.
[30px gap]
"github.com/anshul-garg27/claude-picker" in #89B4FA (blue), 36px monospace, center-aligned, subtle underline in #89B4FA suggesting clickable hyperlink. Most prominent text on slide.
[50px gap]
"Star it. Try it. Tell me what breaks." in #F9E2AF (yellow), 36px, center-aligned, casual tone.

Clean, inviting. Blue URL draws the eye. No busy decorations or emojis.
```

---

# LINKEDIN TEXT POST

**Target**: Personal profile post (not company page)
**Tone**: Developer sharing a personal project — honest, slightly self-deprecating, technically specific
**Length**: ~1,500 characters

---

```
I have 47 Claude Code sessions across 6 projects.

Most of them are unnamed. I couldn't tell you what half of them contain.
And I definitely re-explained context to Claude that I'd already given it three days ago.

So I built claude-picker — a session manager for Claude Code.

It's a two-step fzf picker:
  1. Pick your project (with git branch and session counts)
  2. Pick your session (live preview panel, auto-named if you forgot)

Then you're back in context.

Things it does that no other tool does:

  auth-refactor      $0.47   12.3k tokens
  fix-race-condition $1.23   31.2k tokens
  drizzle-migration  $0.08    2.1k tokens

Per-session cost tracking. Only tool that shows it.

And there's more:

--stats     dashboard with total cost, per-project breakdown, activity timeline
--tree      sessions grouped by project with fork relationships drawn out
--search    full-text across every message in every session in every project
--diff      pick two sessions, compare topics and conversations side-by-side
--pipe      outputs session ID for scripting

Inside the picker:
  Ctrl+B  bookmark (blue pin, top of list)
  Ctrl+E  export conversation to clean markdown
  Ctrl+D  delete in-place
  Ctrl+P  launch from anywhere (shell keybinding)

It's ~800 lines of bash + python3 + fzf 0.58+ + Rich. MIT licensed.
No compiled binary. No build step. Read the source in 20 minutes.

I built this because I was mass-frustrated scrolling session IDs trying to find "that one conversation where I fixed the auth flow." If you use Claude Code daily, you probably know the feeling.

Star it, break it, tell me what's wrong with it:
github.com/anshul-garg27/claude-picker

What's your Claude Code session workflow? Curious if anyone solved this differently.
```

---

### Post Notes

- **Hook line** ("I have 47 Claude Code sessions across 6 projects.") — specific, relatable, fits above the "see more" fold
- **Ends with a question** to drive comments
- **Shows range** of features (cost tracking, stats, tree, search, diff) without listing generically
- **Technical specifics** (~800 lines, fzf 0.58+, Rich, python3) signal credibility
- **"Star it, break it, tell me what's wrong with it"** — honest, invites engagement

---

# LINKEDIN CAROUSEL (12 SLIDES)

**Format**: 1080 x 1350 px (4:5 portrait ratio — outperforms square on mobile)
**Export as**: PDF (each page = one swipeable slide)
**File size**: Keep under 3 MB
**Safe zone**: Keep text within central 880 x 1100 px area; 100px padding on all sides
**Font size**: 28px minimum for body text, 48px+ for headlines
**Colors**: Catppuccin Mocha palette throughout

---

## Slide 1 — Cover (The Hook)

**Headline:**
```
I have 47 Claude Code sessions.

I can't find anything.

So I built a fix.
```

**Design notes:** Dark background (#1E1E2E). "47" in large yellow (#F9E2AF), rest in #CDD6F4. Minimalist. Small "anshul-garg27" credit in bottom-right in muted gray (#6C7086). Slide counter "1/12" top right.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Center-aligned text, 100px padding all sides.

Three lines stacked with 40px spacing:
Line 1: "I have 47 Claude Code sessions." in #CDD6F4, clean sans-serif 56px. "47" in #F9E2AF (yellow), 72px bolder.
Line 2: "I can't find anything." in #CDD6F4, 52px.
Line 3: "So I built a fix." in #A6E3A1 (green), 56px bolder.

Bottom right: "anshul-garg27" in #6C7086, 20px.
Top right: "1/12" in #6C7086, 18px.

Minimalist. No images, decorations, or emojis.
```

---

## Slide 2 — The Problem

**Headline:**
```
The Claude Code session problem
```

**Body:**
```
Sessions pile up fast.

- Unnamed conversations everywhere
- No way to preview before resuming
- Re-explaining context you already gave
- No idea what each session cost you
- Can't search across conversations
- No way to see fork relationships
- No bookmarks for important sessions

Sound familiar?
```

**Design notes:** Background #1E1E2E. Headline in purple (#CBA6F7). Bullet points in #CDD6F4 with peach (#FAB387) dash markers. "Sound familiar?" in yellow (#F9E2AF). Monospace for bullets.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). 100px padding all sides.

Top: "The Claude Code session problem" in #CBA6F7 (purple), bold sans-serif 52px, left-aligned.
[40px gap]
Seven bullet points, left-aligned, #FAB387 (peach) dash markers, text in #CDD6F4 monospace 30px, 32px line spacing:
- "Unnamed conversations everywhere"
- "No way to preview before resuming"
- "Re-explaining context you already gave"
- "No idea what each session cost you"
- "Can't search across conversations"
- "No way to see fork relationships"
- "No bookmarks for important sessions"
[50px gap]
"Sound familiar?" in #F9E2AF (yellow), 42px, left-aligned.

Top right: "2/12" in #6C7086. No emojis.
```

---

## Slide 3 — Introducing the Solution

**Headline:**
```
claude-picker
```

**Subhead:**
```
Browse, preview, and resume
Claude Code sessions from your terminal.
```

**Body:**
```
bash + python3 + fzf 0.58+ + Rich
~800 lines. MIT licensed.
Unix philosophy: do one thing well.
```

**Design notes:** "claude-picker" in large purple (#CBA6F7), centered. Subhead in green (#A6E3A1). Body in #CDD6F4. Terminal window mockup below: #313244 background, three title bar dots, "$ claude-picker" with blinking cursor.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Center-aligned, 100px padding.

Top: "claude-picker" in #CBA6F7 (purple), bold monospace 108px hero.
[24px gap]
"Browse, preview, and resume" in #A6E3A1 (green), 36px.
"Claude Code sessions from your terminal." in #CDD6F4, 32px.
[50px gap]
Terminal window mockup 800x160px: rounded rectangle, #313244 background, 1px #45475A border. Title bar dots #F38BA8, #F9E2AF, #A6E3A1. Inside: "$ claude-picker" in #CDD6F4 monospace 28px, cursor block #CBA6F7.
[40px gap]
"bash + python3 + fzf 0.58+ + Rich" in #CDD6F4, 24px.
"~800 lines. MIT licensed." in #F9E2AF (yellow), 24px.

Top right: "3/12" in #6C7086. No emojis.
```

---

## Slide 4 — Step 1: Pick Your Project

**Headline:**
```
Step 1: Pick your project
```

**Body:**
```
All directories with Claude sessions.
Git branch + session count + activity bar.

  > architex             main        47 ██████
    design-system        feat/v2     23 ████
    infra-terraform      prod         8 ██
    docs-site            main         3 █
```

**Design notes:** fzf-styled list. ">" in green (#A6E3A1), selected row on #45475A highlight bar. Project names in #CDD6F4, branch in #89B4FA, count in #F9E2AF, activity bars in #CBA6F7.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). 100px padding.

Top: "Step 1: Pick your project" in #CBA6F7 (purple), bold 52px, left-aligned.
[24px gap]
"All directories with Claude sessions." in #CDD6F4, 28px.
"Git branch + session count + activity bar." in #A6E3A1 (green), 28px.
[40px gap]
Terminal mockup 880x440px: #313244 background, #45475A border, three title bar dots.

Inside, fzf-style list monospace 28px:
Row 1 (selected, highlighted on #45475A bar):
"> architex" with ">" in #A6E3A1, "architex" in #CDD6F4 | "main" in #89B4FA | "47" in #F9E2AF | "██████" in #CBA6F7
Row 2: "  design-system" #6C7086 | "feat/v2" #89B4FA | "23" #F9E2AF | "████" #CBA6F7
Row 3: "  infra-terraform" #6C7086 | "prod" #89B4FA | "8" #F9E2AF | "██" #CBA6F7
Row 4: "  docs-site" #6C7086 | "main" #89B4FA | "3" #F9E2AF | "█" #CBA6F7

Top right: "4/12" in #6C7086. No emojis.
```

---

## Slide 5 — Step 2: Pick Your Session

**Headline:**
```
Step 2: Pick your session
```

**Body:**
```
Named sessions on top. Preview panel right.
Token count, cost, age warnings, auto-names.

  ── saved ──
  📌 auth-refactor       $0.47  12.3k  2d
  ── recent ──
  > drizzle-migration    $0.08   2.1k  5m
    fix-race-condition   $1.23  31.2k  1h
    debug-websockets     $2.87  72.4k  7d ⚠
```

**Design notes:** Section headers in muted gray. Bookmark pin in blue. Selected row highlighted. Dollar in green, tokens in blue. Age warning peach. Vertical divider suggests preview panel on right.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). 100px padding.

Top: "Step 2: Pick your session" in #CBA6F7 (purple), bold 52px, left-aligned.
[24px gap]
"Named sessions on top. Preview panel right." in #CDD6F4, 26px.
"Token count, cost, age warnings, auto-names." in #A6E3A1 (green), 26px.
[40px gap]
Terminal mockup 880x520px: #313244 background, #45475A border, title bar dots.

Inside, fzf-style list monospace 26px:
"── saved ──" in #6C7086 italic, center-spaced dashes
Row: Blue pin icon "▣" in #89B4FA + " auth-refactor" #CDD6F4 + " $0.47" #A6E3A1 + " 12.3k" #89B4FA + " 2d" #CDD6F4
"── recent ──" in #6C7086 italic
Row (selected, highlighted #45475A): "> drizzle-migration" #CDD6F4 + " $0.08" #A6E3A1 + " 2.1k" #89B4FA + " 5m" #CDD6F4
Row: "  fix-race-condition" #6C7086 + " $1.23" #A6E3A1 + " 31.2k" #89B4FA + " 1h" #CDD6F4
Row: "  debug-websockets" #6C7086 + " $2.87" #A6E3A1 + " 72.4k" #89B4FA + " 7d" #FAB387 + " ⚠" #FAB387

Thin vertical line 1px #45475A at right third of terminal suggesting preview panel.

Top right: "5/12" in #6C7086. No emojis (the pin ▣ and warning ⚠ are block characters, not emojis).
```

---

## Slide 6 — The Killer Feature (Cost)

**Headline:**
```
Per-session cost tracking
```

**Subhead:**
```
No other Claude tool does this.
```

**Body:**
```
See exactly what each conversation costs.

  auth-refactor        $0.47   12.3k tokens
  fix-race-condition   $1.23   31.2k tokens
  drizzle-migration    $0.08    2.1k tokens
  debug-websockets     $2.87   72.4k tokens

Stop guessing. Start knowing.
```

**Design notes:** THE MOST IMPORTANT SLIDE. "Per-session cost tracking" in large peach (#FAB387). "No other Claude tool does this." in yellow (#F9E2AF). Cost table in terminal frame. Dollar column glows green.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Center-aligned, 100px padding.

Top: "Per-session cost tracking" in #FAB387 (peach), bold 64px center.
[20px gap]
"No other Claude tool does this." in #F9E2AF (yellow), 36px bold center.
[40px gap]
Terminal window 880x360px: #313244 background, #45475A border, three title bar dots.

Inside, four rows monospace 30px, left-aligned 32px padding:
"auth-refactor       " #CDD6F4 | "$0.47" #A6E3A1 | " 12.3k tokens" #89B4FA
"fix-race-condition  " #CDD6F4 | "$1.23" #A6E3A1 | " 31.2k tokens" #89B4FA
"drizzle-migration   " #CDD6F4 | "$0.08" #A6E3A1 | "  2.1k tokens" #89B4FA
"debug-websockets    " #CDD6F4 | "$2.87" #A6E3A1 | " 72.4k tokens" #89B4FA

Dollar column has subtle green glow effect or slightly larger font weight.
[40px gap]
"Stop guessing. Start knowing." in #A6E3A1 (green), bold 40px center.

Top right: "6/12" in #6C7086. No emojis.
```

---

## Slide 7 — Stats Dashboard

**Headline:**
```
claude-picker --stats
```

**Subhead:**
```
Full dashboard. One command.
```

**Body:**
```
  Total sessions:    142
  Total tokens:     2.4M
  Total cost:    $38.27

  architex        ████████ $12.40
  design-system   ██████   $8.92
  infra           ████     $5.13

  Top 5 sessions by usage:
    debug-websockets    $2.87
    fix-race-condition  $1.23
    ...
```

**Design notes:** Dashboard view inside terminal. Totals row with colored values. Per-project bars in purple. Top sessions ranked.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). 100px padding.

Top: "claude-picker --stats" in #CDD6F4 monospace 44px, "--stats" in #F9E2AF (yellow).
[16px gap]
"Full dashboard. One command." in #A6E3A1 (green), 28px.
[40px gap]
Terminal window 880x700px: #313244, #45475A border, three title bar dots.

Inside, monospace 26px, left-aligned 32px padding:

Section — Totals:
"Total sessions:    " #CDD6F4 + "142" #89B4FA
"Total tokens:     " #CDD6F4 + "2.4M" #F9E2AF
"Total cost:    " #CDD6F4 + "$38.27" #A6E3A1
[20px gap]

Section — Per-project:
"architex        " #CDD6F4 + "████████" #CBA6F7 + " $12.40" #A6E3A1
"design-system   " #CDD6F4 + "██████" #CBA6F7 + " $8.92" #A6E3A1
"infra           " #CDD6F4 + "████" #CBA6F7 + " $5.13" #A6E3A1
[20px gap]

Section — Top sessions:
"Top 5 sessions by usage:" #6C7086
"  debug-websockets    " #CDD6F4 + "$2.87" #A6E3A1
"  fix-race-condition  " #CDD6F4 + "$1.23" #A6E3A1
"  ..." #6C7086

Top right: "7/12" in #6C7086. Block characters render cleanly. No emojis.
```

---

## Slide 8 — Session Tree with Forks

**Headline:**
```
claude-picker --tree
```

**Subhead:**
```
See how your sessions branched.
```

**Body:**
```
architex/
  ├─ auth-refactor
  │   └─ auth-refactor-retry (fork)
  │       └─ auth-refactor-v3 (fork)
  ├─ drizzle-migration
  └─ fix-race-condition

design-system/
  └─ button-variants
      └─ button-variants-a11y (fork)
```

**Design notes:** Tree view. Project names in purple. Tree characters in muted gray. Session names in #CDD6F4. "(fork)" labels in peach to highlight branching.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). 100px padding.

Top: "claude-picker --tree" in #CDD6F4 monospace 44px, "--tree" in #F9E2AF.
[16px gap]
"See how your sessions branched." in #A6E3A1 (green), 28px.
[40px gap]
Terminal window 880x560px: #313244, #45475A border, three title bar dots.

Inside, monospace 28px, left-aligned 32px padding:

"architex/" in #CBA6F7 (purple), bold
"  ├─ auth-refactor" — tree chars in #6C7086, name in #CDD6F4
"  │   └─ auth-refactor-retry" — tree #6C7086, name #CDD6F4, " (fork)" in #FAB387 (peach)
"  │       └─ auth-refactor-v3" — tree #6C7086, name #CDD6F4, " (fork)" in #FAB387
"  ├─ drizzle-migration" — tree #6C7086, name #CDD6F4
"  └─ fix-race-condition" — tree #6C7086, name #CDD6F4
[20px gap]
"design-system/" in #CBA6F7 bold
"  └─ button-variants" — tree #6C7086, name #CDD6F4
"      └─ button-variants-a11y" — tree #6C7086, name #CDD6F4, " (fork)" in #FAB387

Top right: "8/12" in #6C7086. Box-drawing characters must render cleanly. No emojis.
```

---

## Slide 9 — Full-Text Search Across All Projects

**Headline:**
```
claude-picker --search
```

**Subhead:**
```
Every message. Every session. Every project.
```

**Body:**
```
$ claude-picker --search "race condition"

architex   fix-race-condition
  > "race condition in the websocket
    reconnect logic when both pods..."

infra      deploy-debug
  > "might be a race condition when
    both pods start simultaneously"

Select -> auto-cd to project -> resume.
```

**Design notes:** Search command with quoted query. Results grouped by project. Match snippets indented with ">" prefix. Project names blue, session names purple.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). 100px padding.

Top: "claude-picker --search" in #CDD6F4 monospace 44px, "--search" in #F9E2AF.
[16px gap]
"Every message. Every session. Every project." in #A6E3A1 (green), 26px.
[30px gap]
Terminal window 880x680px: #313244, #45475A border, three title bar dots.

Inside, monospace 26px, left-aligned 32px padding:

"$ claude-picker --search \"race condition\"" in #CDD6F4, "--search" #F9E2AF, "\"race condition\"" in #A6E3A1.
[24px gap]

Result 1:
"architex   fix-race-condition" — "architex" in #89B4FA (blue), session in #CBA6F7 (purple)
"  > \"race condition in the websocket" — ">" in #A6E3A1 (green), text in #CDD6F4
"    reconnect logic when both pods...\"" in #CDD6F4
[20px gap]

Result 2:
"infra      deploy-debug" — "infra" in #89B4FA, session in #CBA6F7
"  > \"might be a race condition when" — ">" in #A6E3A1, text in #CDD6F4
"    both pods start simultaneously\"" in #CDD6F4
[30px gap]

"Select -> auto-cd to project -> resume." in #FAB387 (peach), 24px italic.

Top right: "9/12" in #6C7086. No emojis.
```

---

## Slide 10 — Session Diff

**Headline:**
```
claude-picker --diff
```

**Subhead:**
```
Pick two. Compare side-by-side.
```

**Body:**
```
┌─ auth-refactor ─────┬─ auth-refactor-v3 ──┐
│ Topic: JWT refresh  │ Topic: JWT + CSRF   │
│                     │                     │
│ you: how do I rot.. │ you: how do I rot.. │
│ ai: rotate the key  │ ai: rotate + add    │
│                     │      CSRF token     │
│ 12.3k tokens        │ 18.7k tokens        │
│ $0.47               │ $0.71               │
└─────────────────────┴─────────────────────┘
```

**Design notes:** Two-panel diff view. Topics highlighted. "you:" in cyan, "ai:" in yellow. Different content shown in peach to signal the divergence. Clean split layout.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). 100px padding.

Top: "claude-picker --diff" in #CDD6F4 monospace 44px, "--diff" in #F9E2AF.
[16px gap]
"Pick two. Compare side-by-side." in #A6E3A1 (green), 28px.
[30px gap]
Terminal window 880x680px: #313244, #45475A border, three title bar dots.

Inside, two-panel diff layout with box-drawing frame in #45475A, monospace 22px:

Header row: "┌─ auth-refactor ─────┬─ auth-refactor-v3 ──┐" frame #45475A, session names #CBA6F7 (purple)
Row: "│ Topic: JWT refresh  │ Topic: JWT + CSRF   │" frame #45475A, "Topic:" label #6C7086, topics in #CDD6F4
[blank row with just borders]
Row: "│ you: how do I rot.. │ you: how do I rot.. │" frame #45475A, "you:" in #89B4FA (cyan), text #CDD6F4
Row: "│ ai: rotate the key  │ ai: rotate + add    │" "ai:" in #F9E2AF (yellow), text on left #CDD6F4, text on right #FAB387 (peach — divergent)
Row: "│                     │      CSRF token     │" right side in #FAB387
[blank row]
Row: "│ 12.3k tokens        │ 18.7k tokens        │" tokens in #89B4FA
Row: "│ $0.47               │ $0.71               │" cost in #A6E3A1
Footer: "└─────────────────────┴─────────────────────┘" frame #45475A

Top right: "10/12" in #6C7086. Box-drawing must render cleanly. No emojis.
```

---

## Slide 11 — Everything It Does

**Headline:**
```
Every feature
```

**Body (two columns, green checkmarks):**
```
Core flow                      Advanced
 Two-step fzf picker            --search across all projects
 Conversation preview panel     --stats dashboard
 Rich-formatted Python          --tree with fork detection
 Catppuccin Mocha 24-bit        --diff compare two sessions
                                --pipe for scripting
In-picker shortcuts
 Ctrl+B bookmark               Smart display
 Ctrl+E export to markdown      Token + cost estimates
 Ctrl+D delete in-place         Auto-named sessions
 Ctrl+P launch from anywhere    Git branch per project
                                Age warnings (7d/30d)
Integrations                    Relative time
 Warp + menu tab config
 /claude-picker slash command  Filtering
 Reads entrypoint field         Claude CLI sessions only
```

**Design notes:** Two-column grid. Four section headers in purple, bold. Green checkmarks before each item. Text in #CDD6F4. Monospace. Compact, scannable.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). 100px padding.

Top: "Every feature" in #CBA6F7 (purple), bold sans-serif 52px, left-aligned.
[30px gap]

Two-column grid. Column 1 width 420px, column 2 width 420px, 40px gutter. Monospace 20px throughout, tight 26px line spacing. Green checkmark "✓" in #A6E3A1 before each item.

Column 1 (left):
Section header "Core flow" #CBA6F7 bold
✓ "Two-step fzf picker" #CDD6F4
✓ "Conversation preview panel" #CDD6F4
✓ "Rich-formatted Python" #CDD6F4
✓ "Catppuccin Mocha 24-bit" #CDD6F4
[12px gap]
Section header "In-picker shortcuts" #CBA6F7 bold
✓ "Ctrl+B bookmark" #CDD6F4
✓ "Ctrl+E export to markdown" #CDD6F4
✓ "Ctrl+D delete in-place" #CDD6F4
✓ "Ctrl+P launch from anywhere" #CDD6F4
[12px gap]
Section header "Integrations" #CBA6F7 bold
✓ "Warp + menu tab config" #CDD6F4
✓ "/claude-picker slash command" #CDD6F4
✓ "Reads entrypoint field" #CDD6F4

Column 2 (right):
Section header "Advanced" #CBA6F7 bold
✓ "--search across all projects" #CDD6F4
✓ "--stats dashboard" #CDD6F4
✓ "--tree with fork detection" #CDD6F4
✓ "--diff compare two sessions" #CDD6F4
✓ "--pipe for scripting" #CDD6F4
[12px gap]
Section header "Smart display" #CBA6F7 bold
✓ "Token + cost estimates" #CDD6F4
✓ "Auto-named sessions" #CDD6F4
✓ "Git branch per project" #CDD6F4
✓ "Age warnings (7d/30d)" #CDD6F4
✓ "Relative time" #CDD6F4
[12px gap]
Section header "Filtering" #CBA6F7 bold
✓ "Claude CLI sessions only" #CDD6F4

Top right: "11/12" in #6C7086. No emojis — the check marks are "✓" Unicode.
```

---

## Slide 12 — CTA (Final Slide)

**Headline:**
```
Try it now
```

**Body:**
```
github.com/anshul-garg27/claude-picker

MIT licensed. Works today.

  Star it if it's useful.
  Open an issue if it breaks.
  Fork it if you want more.

Built by @anshul-garg27
```

**Design notes:** "Try it now" in large green. URL in blue, clickable-styled. Three action items with colored bullets: star=yellow, issue=peach, fork=purple. Credit in muted gray. GitHub logo outline.

**Gemini AI Pro prompt:**
```
Create a professional LinkedIn carousel slide at 1080x1350 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Center-aligned, 100px padding.

Top: "Try it now" in #A6E3A1 (green), bold 80px centered.
[30px gap]
Simplified GitHub octocat outline in #CDD6F4, 60x60px centered.
[20px gap]
"github.com/anshul-garg27/claude-picker" in #89B4FA (blue), monospace 34px centered, subtle underline in #89B4FA suggesting hyperlink.
[30px gap]
"MIT licensed. Works today." in #CDD6F4, 30px centered.
[40px gap]
Three action items, center-aligned with colored bullet dots (14px circles, 12px gap to text), 32px line spacing:
- Yellow dot #F9E2AF + "Star it if it's useful." in #CDD6F4 28px
- Peach dot #FAB387 + "Open an issue if it breaks." in #CDD6F4 28px
- Purple dot #CBA6F7 + "Fork it if you want more." in #CDD6F4 28px
[50px gap]
"Built by @anshul-garg27" in #6C7086 (muted gray), 22px center.

Top right: "12/12" in #6C7086. No emojis.
```

---

# HASHTAG STRATEGY

## LinkedIn Hashtags (use 3-5 per post, quality over quantity)

**Primary (high relevance, medium volume):**
- #ClaudeCode
- #DeveloperTools
- #OpenSource
- #CLI

**Secondary (broader reach):**
- #SoftwareEngineering
- #DevTools
- #AITools
- #Productivity

**Niche (targeted discovery):**
- #TerminalTools
- #BashScript
- #DevExperience
- #AIAssistant

**Recommended combination for the LinkedIn post:**
```
#ClaudeCode #DeveloperTools #OpenSource #CLI #DevExperience
```

**Recommended combination for the LinkedIn carousel:**
```
#ClaudeCode #OpenSource #DeveloperTools #AITools
```

**Why this selection:** LinkedIn's algorithm uses hashtags as topic signals, not discovery mechanisms like Instagram. Fewer, more relevant hashtags outperform hashtag-stuffing. #ClaudeCode is niche enough to own, and #OpenSource and #DeveloperTools connect to large existing communities.

## Instagram Hashtags (use 8-12 per story/post)

**Primary:**
- #developer #coding #programming #opensource #coder

**Secondary:**
- #devtools #terminal #linux #commandline #bash

**Niche:**
- #claudecode #fzf #sessionmanager #devproductivity #codingtools

**Recommended combination for Instagram:**
```
#developer #coding #opensource #devtools #terminal
#claudecode #commandline #codingtools #programming #bash
```

---

# POSTING SCHEDULE

## Optimal Timing (based on 2026 data analysis of 4.8M+ posts)

### LinkedIn

| Priority | Day | Time (ET) | Rationale |
|----------|-----|-----------|-----------|
| 1st choice | Tuesday | 10:00 AM | Peak developer engagement window, pre-lunch scroll |
| 2nd choice | Wednesday | 10:00 AM | Second-highest mid-week engagement |
| 3rd choice | Thursday | 8:30 AM | Catches early-morning deep-work break |

**Post the carousel and text post on DIFFERENT days** — do not cannibalize your own reach.

**Recommended launch sequence:**
1. **Tuesday 10 AM ET** — Post the LinkedIn carousel (highest visual impact first)
2. **Wednesday 10 AM ET** — Post the LinkedIn text post (catches people who missed the carousel)
3. **Thursday** — Engage heavily in comments on both posts during the day

### Instagram Stories

| Priority | Day | Time (ET) | Rationale |
|----------|-----|-----------|-----------|
| 1st choice | Tuesday | 12:00 PM | Lunch break scrolling |
| 2nd choice | Wednesday | 6:00 PM | Post-work decompression |

**Post all 10 story slides at once** — they form a narrative sequence. Do NOT spread across days.

### Cross-Platform Sequence

```
Tuesday   10:00 AM ET  →  LinkedIn carousel (12 slides)
Tuesday   12:00 PM ET  →  Instagram Stories (all 10 slides)
Wednesday 10:00 AM ET  →  LinkedIn text post
Wednesday  6:00 PM ET  →  Instagram feed post (optional, repurpose slide 6)
Thursday   all day     →  Comment engagement on all platforms
```

### Days to Avoid
- **Saturday and Sunday** — worst LinkedIn engagement by far
- **Monday** — people are in catch-up mode, not discovery mode
- **Friday afternoon** — check-out mode

---

# CATPPUCCIN MOCHA COLOR REFERENCE

For quick reference when creating any additional assets:

| Name | Hex | Usage |
|------|-----|-------|
| Base (Background) | #1E1E2E | All backgrounds |
| Text | #CDD6F4 | Primary body text |
| Subtext/Muted | #6C7086 | Muted text, deemphasized elements, meta info |
| Surface 0 | #313244 | Terminal backgrounds, cards, containers |
| Surface 1 / Border | #45475A | Borders, dividers, highlight bars |
| Lavender/Purple (brand) | #CBA6F7 | Headlines, brand, claude-picker name, bars |
| Green (success) | #A6E3A1 | Success, costs, checkmarks, CTAs, fzf selector |
| Yellow (emphasis) | #F9E2AF | Numbers, stats, emphasis, command flags |
| Blue (links) | #89B4FA | Links, token counts, URLs, bookmark pin, project names in search |
| Peach (secondary) | #FAB387 | Secondary emphasis, fork labels, age warnings (7d+), diff divergence |
| Red (warning) | #F38BA8 | Terminal title bar dot, age warnings (30d+), deletion confirm |

---

# QUICK-START CHECKLIST

## Before You Post

- [ ] Generate all images using the Gemini prompts above
- [ ] Preview Instagram Stories on a phone (not desktop) — check safe zones
- [ ] Export LinkedIn carousel as a single PDF, verify each page at 1080x1350
- [ ] Test the GitHub URL is live and README is polished
- [ ] Prepare 2-3 reply comments for your own LinkedIn post (FAQ answers, technical details)
- [ ] Have the repo starred by 3-5 friends/colleagues before posting (social proof)

## Engagement Strategy (First 90 Minutes Are Critical)

- [ ] Reply to every comment within the first 2 hours
- [ ] Like every comment (signals engagement to algorithm)
- [ ] Share the LinkedIn post to 2-3 relevant Slack/Discord communities after posting
- [ ] Cross-post to relevant subreddits: r/ClaudeAI, r/commandline, r/linux
- [ ] If anyone asks a question, reply with a follow-up question to keep the thread going

## Follow-Up Content (Days 2-7)

- [ ] Day 2: Post a short "behind the build" story (why bash over Go/Rust)
- [ ] Day 3: Share a specific `--stats` or `--tree` screen recording
- [ ] Day 5: Post metrics update ("X stars in 3 days — here's what people asked for")
- [ ] Day 7: Share a feature request poll or "what should I build next"

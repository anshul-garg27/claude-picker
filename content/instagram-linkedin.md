# claude-picker: Social Media Launch Content
## Complete Content Kit for Instagram Stories + LinkedIn

> GitHub: github.com/anshul-garg27/claude-picker
> Generated: April 2026

---

# TABLE OF CONTENTS

1. [Research Summary & Strategy](#research-summary--strategy)
2. [Instagram Stories Content (7 Slides)](#instagram-stories-content)
3. [LinkedIn Text Post](#linkedin-text-post)
4. [LinkedIn Carousel (10 Slides)](#linkedin-carousel-10-slides)
5. [Hashtag Strategy](#hashtag-strategy)
6. [Posting Schedule](#posting-schedule)
7. [Image Generation Prompts (Gemini AI Pro)](#image-generation-prompts)

---

# RESEARCH SUMMARY & STRATEGY

## Platform Analysis

### Instagram for Developers — Does It Work?

Short answer: yes, but with caveats. Tech content is a growing niche on Instagram (over 60% of creator income comes from brand collaborations in tech, lifestyle, and similar niches). Instagram's 2026 "Year of Raw Content" shift means the algorithm now favors authenticity over polish — which benefits a solo developer sharing real tools over a corporate marketing team. That said, Instagram is a secondary channel for devtools. Use it for visual appeal, brand awareness, and reaching developers who cross-browse (many senior devs in their 30s-40s use Instagram daily). Stories are ideal for quick, punchy content that disappears — low commitment, high curiosity.

### LinkedIn for Developer Tool Launches — The Primary Channel

LinkedIn carousels generate 3.5x more engagement than text-only posts (6.60% avg engagement rate vs ~2% for text). PDF carousels have the longest lifespan of any LinkedIn format: 1-2 weeks of active reach. Over 67% of LinkedIn usage is mobile, and carousels feel native to touch-swipe behavior. The first 60-90 minutes determine 70% of your post's total reach — hook quality and timing are critical. The algorithm in 2026 measures "dwell time" — how long someone spends on your content. A 10-slide carousel that someone fully swipes is an enormous engagement signal.

## Competitive Positioning

| Feature | claude-picker | claude-history (Rust) | Claude Squad (Go) |
|---------|--------------|----------------------|-------------------|
| Per-session token/cost display | YES (unique) | No | No |
| Weight | 432 lines bash+python | Compiled binary | Compiled binary |
| Philosophy | Unix pipes, fzf | Standalone | Standalone |
| Named sessions | Yes | Yes | Yes |
| Full-text search | Yes | Partial | No |
| Dependencies | bash, python, fzf | None (Rust binary) | None (Go binary) |

**Our angle in all content**: Lightest weight. Unix philosophy. ONLY tool that shows you what each session costs.

---

# INSTAGRAM STORIES CONTENT

**Format**: 1080 x 1920 px (9:16 ratio)
**Safe zone**: Keep critical content within 1080 x 1390 px (centered), avoiding top 250px and bottom 280px
**Color palette**: Catppuccin Mocha throughout

## Story Slide 1 of 7 — The Hook

**Text on screen:**
```
You have 47 Claude Code sessions.

Which one had that auth fix?

Good luck finding it.
```

**Visual direction:** Dark background (#1E1E2E). Text centered. "47" in large yellow (#F9E2AF). "Good luck finding it." in smaller gray, slightly sardonic. No images — just text on dark. Minimalist. The emptiness IS the point.

**Swipe-up text:** "Swipe to see the fix -->"

---

## Story Slide 2 of 7 — The Problem

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

**Visual direction:** Same dark background (#1E1E2E). Text in #CDD6F4. "burn tokens" in peach (#FAB387). "waste money" in red-ish peach. Left-aligned text. Each line appears stacked with breathing room. Terminal-style monospace font feel.

---

## Story Slide 3 of 7 — Introducing claude-picker

**Text on screen:**
```
claude-picker

Browse. Preview. Resume.
Your Claude Code sessions.

fzf-powered. 432 lines.
bash + python + fzf.
```

**Visual direction:** Center-aligned. "claude-picker" in large purple (#CBA6F7) as the hero text. "Browse. Preview. Resume." in green (#A6E3A1). The "432 lines" stat in yellow (#F9E2AF). Below it, a subtle terminal window outline (rounded rectangle in #313244) with a fake blinking cursor. Background #1E1E2E.

---

## Story Slide 4 of 7 — The Killer Feature

**Text on screen:**
```
No other tool shows you this:

  auth-refactor    $0.47  12.3k tokens
  fix-ci-pipeline  $1.23  31.2k tokens
  add-tests        $0.08   2.1k tokens

Per-session cost tracking.
Know exactly what you're spending.
```

**Visual direction:** This is the money slide (literally). Background #1E1E2E. The three session lines styled like a terminal output, monospace font, inside a terminal window frame (#313244 border). Dollar amounts in green (#A6E3A1). Token counts in blue (#89B4FA). Session names in #CDD6F4. "No other tool shows you this" in yellow (#F9E2AF) at top. "Per-session cost tracking." bold, in peach (#FAB387). This slide should feel like looking at a real terminal.

---

## Story Slide 5 of 7 — How It Works

**Text on screen:**
```
Step 1: Pick your project
Step 2: Pick your session
Step 3: You're back in context

Full preview panel.
Fuzzy search everything.
Ctrl+D to delete.
```

**Visual direction:** Three steps stacked vertically with step numbers in purple circles (#CBA6F7). Each step text in #CDD6F4. Below, the three feature bullets in a slightly different style — green (#A6E3A1) checkmarks before each. Background #1E1E2E. Clean, scannable layout.

---

## Story Slide 6 of 7 — The Comparison

**Text on screen:**
```
Claude Squad     → 6.6k stars, Go, heavy
claude-history   → Rust binary, no costs
claude-picker    → 432 lines, shows costs,
                   Unix philosophy

Sometimes less is more.
```

**Visual direction:** Three rows. First two in muted gray (#6C7086). The claude-picker row in bright green (#A6E3A1) with a subtle glow/highlight effect. "Sometimes less is more." in purple (#CBA6F7) at the bottom, slightly larger. Background #1E1E2E. The visual hierarchy should make claude-picker pop.

---

## Story Slide 7 of 7 — CTA

**Text on screen:**
```
MIT licensed. Works now.

github.com/anshul-garg27/claude-picker

Star it. Try it. Tell me what breaks.
```

**Visual direction:** GitHub logo (simplified, white outline) at the top. URL in blue (#89B4FA), styled to look like a clickable link. "Star it. Try it. Tell me what breaks." in yellow (#F9E2AF) — casual, honest, not corporate. Background #1E1E2E. If possible, add the link sticker pointing to the repo.

**Instagram link sticker:** github.com/anshul-garg27/claude-picker

---

# LINKEDIN TEXT POST

**Target**: Personal profile post (not company page)
**Tone**: Developer sharing a personal project — honest, slightly self-deprecating, technically specific
**Length**: ~1,300 characters (under LinkedIn's 3,000 char limit, long enough for substance)

---

```
I have 47 Claude Code sessions across 6 projects.

Most of them are unnamed. I couldn't tell you what half of them contain.
And I definitely re-explained context to Claude that I'd already given it three days ago.

So I built claude-picker — a session manager for Claude Code.

It's a two-step fzf picker:
  1. Pick your project
  2. Pick your session (with a live preview panel)

Then you're back in your conversation with full context.

The thing I'm most proud of: per-session cost tracking.

  auth-refactor      $0.47   12.3k tokens
  fix-ci-pipeline    $1.23   31.2k tokens
  add-tests          $0.08    2.1k tokens

No other Claude session tool shows you this. Not claude-history (Rust, 197 stars). Not Claude Squad (Go, 6.6k stars). Nobody.

Other things it does:
- Named sessions (claude --name "feature-x")
- Full-text search across all sessions
- Auto-names unnamed sessions so you stop guessing
- Ctrl+D to delete sessions you're done with
- ANSI 256-color UI that works in any terminal

It's 432 lines of bash + python + fzf. MIT licensed.

I built this because I was mass-frustrated with scrolling through session IDs trying to find "that one conversation where I fixed the auth flow." If you use Claude Code daily, you probably know the feeling.

Star it, break it, tell me what's wrong with it:
github.com/anshul-garg27/claude-picker

What's your Claude Code session management workflow? I'm curious if anyone else has solved this differently.
```

---

### Post Notes

- **Hook line** ("I have 47 Claude Code sessions across 6 projects.") — specific number, relatable pain, fits above the "see more" fold
- **Ends with a question** to drive comments
- **Mentions competitors by name** — this is intentional; it shows confidence and helps with search/discovery
- **Technical specifics** (432 lines, fzf, bash+python) signal credibility to developer audience
- **"Star it, break it, tell me what's wrong with it"** — honest, invites engagement, not corporate

---

# LINKEDIN CAROUSEL (10 SLIDES)

**Format**: 1080 x 1350 px (4:5 portrait ratio — outperforms square on mobile)
**Export as**: PDF (each page = one swipeable slide)
**File size**: Keep under 3 MB
**Safe zone**: Keep text within central 880 x 1100 px area; 100px padding on all sides
**Font size**: 28px minimum for body text, 48px+ for headlines
**Colors**: Catppuccin Mocha palette throughout — this is your brand identity in the carousel

---

## Slide 1 — Cover (The Hook)

**Headline:**
```
I have 47 Claude Code sessions.

I can't find anything.

So I built a fix.
```

**Design notes:** Dark background (#1E1E2E). "47" in large yellow (#F9E2AF), rest of text in #CDD6F4. Minimalist — no images, no decorations. Just text and dark space. The starkness is the hook. Small "anshul-garg27" credit in bottom-right corner in muted gray (#6C7086).

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

Sound familiar?
```

**Design notes:** Background #1E1E2E. Headline in purple (#CBA6F7). Bullet points in #CDD6F4 with red/peach (#FAB387) dash markers. "Sound familiar?" in yellow (#F9E2AF) at the bottom — slightly italicized feel. Terminal-style monospace font for the bullets.

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
bash + python + fzf
432 lines. MIT licensed.
Unix philosophy: do one thing well.
```

**Design notes:** "claude-picker" in large purple (#CBA6F7), centered, dominant. Subhead in green (#A6E3A1). Body text in #CDD6F4. Below the text, a minimal terminal window mockup — rounded rectangle with #313244 background, three colored dots (red, yellow, green) in the title bar. The terminal shows: `$ claude-picker` with a blinking cursor. Background #1E1E2E.

---

## Slide 4 — How It Works (Step 1)

**Headline:**
```
Step 1: Pick your project
```

**Body:**
```
Two-step fzf picker.

First, you see all your projects.
Fuzzy search to filter instantly.

  > my-api
    frontend-app
    infra-scripts
    docs-site
```

**Design notes:** Background #1E1E2E. Headline in purple (#CBA6F7). The project list styled as fzf output — ">" selector in green (#A6E3A1), selected item highlighted with a #313244 background bar, other items in muted #6C7086. The fzf aesthetic should feel authentic to developers who use it daily.

---

## Slide 5 — How It Works (Step 2)

**Headline:**
```
Step 2: Pick your session
```

**Body:**
```
Every session shows:
- Conversation preview (right panel)
- Token count + cost
- Last modified time
- Auto-generated name

  > fix-auth-flow     $0.47  12.3k tok
    add-rate-limiting  $1.23  31.2k tok
    refactor-db-layer  $0.08   2.1k tok
```

**Design notes:** Same fzf-style list as Slide 4. Dollar amounts in green (#A6E3A1). Token counts in blue (#89B4FA). Session names in #CDD6F4. A vertical divider line in #313244 on the right side suggests the preview panel. Headline in purple (#CBA6F7). Background #1E1E2E.

---

## Slide 6 — The Killer Feature

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

  auth-refactor      $0.47   12.3k tokens
  fix-ci-pipeline    $1.23   31.2k tokens
  add-tests          $0.08    2.1k tokens
  debug-websockets   $2.87   72.4k tokens

Stop guessing. Start knowing.
```

**Design notes:** THIS IS THE MOST IMPORTANT SLIDE. "Per-session cost tracking" in large peach (#FAB387). "No other Claude tool does this." in yellow (#F9E2AF) — bold, confident. The session cost table in a terminal frame. Dollar amounts in green (#A6E3A1). Token counts in blue (#89B4FA). "Stop guessing. Start knowing." in green (#A6E3A1) at the bottom. Background #1E1E2E. Consider a subtle glow/highlight around the cost column.

---

## Slide 7 — Full Feature List

**Headline:**
```
Everything it does
```

**Body (use checkmark icons or green bullets):**
```
 Two-step fzf picker (project -> session)
 Live conversation preview panel
 Named sessions (claude --name "feature-x")
 Full-text content search
 Per-session token & cost display
 Auto-naming for unnamed sessions
 Ctrl+D to delete sessions
 ANSI 256-color terminal UI
 Fuzzy search across everything
 Works in any terminal + optional Warp integration
```

**Design notes:** Background #1E1E2E. Headline in purple (#CBA6F7). Each feature preceded by a green (#A6E3A1) checkmark. Text in #CDD6F4. Compact, scannable layout. Two columns if needed. This is the "feature dump" slide — keep it clean and organized. No decoration — let the list speak.

---

## Slide 8 — Competitive Comparison

**Headline:**
```
How it compares
```

**Body (styled as a comparison table):**
```
                    claude-picker  Claude Squad  claude-history
Per-session cost    -------        -----------   --------------
  tracking              YES            No             No
Size                432 lines       Large (Go)    Medium (Rust)
Philosophy          Unix pipes      Standalone    Standalone
Stars               New!            6.6k          197
Install             curl + chmod    go install    cargo install
```

**Design notes:** Background #1E1E2E. Headline in purple (#CBA6F7). Table with columns. claude-picker column highlighted with a subtle green (#A6E3A1) vertical stripe or glow. "YES" for cost tracking in bold green (#A6E3A1). Competitor values in muted gray (#6C7086). "New!" in yellow (#F9E2AF) with a slightly playful feel. The visual bias should clearly favor claude-picker without being dishonest.

---

## Slide 9 — The Philosophy

**Headline:**
```
432 lines of bash + python + fzf
```

**Body:**
```
No compiled binary.
No build step.
No runtime dependency you don't already have.

Read the source in 10 minutes.
Modify it for your workflow.
Pipe it into whatever you want.

This is how Unix tools should work.
```

**Design notes:** Background #1E1E2E. Headline in yellow (#F9E2AF). Body text in #CDD6F4. "This is how Unix tools should work." in purple (#CBA6F7), slightly larger, at the bottom — this is the manifesto line. Minimalist. Maybe a subtle > prompt character in green (#A6E3A1) before the manifesto line.

---

## Slide 10 — CTA (Final Slide)

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

**Design notes:** Background #1E1E2E. "Try it now" in large green (#A6E3A1). URL in blue (#89B4FA), styled to look clickable. The three action items with different colored bullets: star = yellow (#F9E2AF), issue = peach (#FAB387), fork = purple (#CBA6F7). "Built by @anshul-garg27" in muted gray (#6C7086) at the bottom. GitHub logo outline in white near the URL.

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

**Post all 7 story slides at once** — they form a narrative sequence. Do NOT spread across days.

### Cross-Platform Sequence

```
Tuesday   10:00 AM ET  →  LinkedIn carousel
Tuesday   12:00 PM ET  →  Instagram Stories (all 7 slides)
Wednesday 10:00 AM ET  →  LinkedIn text post
Wednesday  6:00 PM ET  →  Instagram feed post (optional, repurpose slide 6)
Thursday   all day     →  Comment engagement on all platforms
```

### Days to Avoid
- **Saturday and Sunday** — worst LinkedIn engagement by far
- **Monday** — people are in catch-up mode, not discovery mode
- **Friday afternoon** — check-out mode

---

# IMAGE GENERATION PROMPTS (GEMINI AI PRO)

All prompts below are designed for Gemini AI Pro image generation. Each prompt specifies the exact Catppuccin Mocha hex codes. These prompts produce images at the specified dimensions.

---

## Instagram Story Prompts (1080 x 1920 px)

### Story Slide 1 — The Hook

```
Create a minimalist Instagram Story image at 1080x1920 pixels.

Background: solid dark color #1E1E2E (Catppuccin Mocha Base).

Centered text layout with generous whitespace. Three lines of text:

Line 1: "You have 47 Claude Code sessions." in color #CDD6F4, clean sans-serif font, medium weight. The number "47" should be in color #F9E2AF (golden yellow) and slightly larger/bolder than the rest of the line.

Line 2: "Which one had that auth fix?" in color #CDD6F4, same font, slightly smaller.

Line 3: "Good luck finding it." in color #6C7086 (muted gray), slightly smaller and italic.

The text should be vertically centered in the safe zone (avoid top 250px and bottom 280px). No images, no decorations, no gradients. Just text on dark background. The emptiness and negative space should feel intentional and dramatic. Monospace or clean sans-serif typeface. No emojis.
```

### Story Slide 2 — The Problem

```
Create a minimalist Instagram Story image at 1080x1920 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Left-aligned text block, positioned in the safe zone (centered vertically, avoiding top 250px and bottom 280px), with 80px left padding.

Text content, each line on its own row with 24px spacing:

"Claude Code doesn't have" — color #CDD6F4, clean monospace font
"a session manager." — color #CDD6F4, same font

[40px gap]

"Your conversations pile up." — color #CDD6F4
"Unnamed. Unsearchable. Forgotten." — color #6C7086 (muted gray)

[40px gap]

"You re-explain context." — color #CDD6F4
"You burn tokens." — color #FAB387 (peach/orange), slightly bolder
"You waste money." — color #FAB387 (peach/orange), bold

No decorations, no images. Terminal-aesthetic monospace typography. Dark, stark, slightly ominous feeling. No emojis.
```

### Story Slide 3 — Introducing claude-picker

```
Create a minimalist Instagram Story image at 1080x1920 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Center-aligned layout in the safe zone.

Top section:
"claude-picker" as the hero text in color #CBA6F7 (purple), large bold monospace font, centered.

Below it (20px gap):
"Browse. Preview. Resume." in color #A6E3A1 (green), medium monospace font.
"Your Claude Code sessions." in color #CDD6F4, slightly smaller.

[60px gap]

Below that, a terminal window mockup:
- Rounded rectangle with background #313244 and subtle border in #45475A
- Three small colored dots in the title bar: #F38BA8 (red), #F9E2AF (yellow), #A6E3A1 (green)
- Inside the terminal: "$ claude-picker" in #CDD6F4 with a blinking cursor block in #CBA6F7

[40px gap]

Bottom section:
"fzf-powered. 432 lines." in color #F9E2AF (yellow), small text.
"bash + python + fzf" in color #6C7086 (muted gray), smallest text.

No photographs, no gradients. Clean developer aesthetic. No emojis.
```

### Story Slide 4 — The Killer Feature (Cost Tracking)

```
Create a minimalist Instagram Story image at 1080x1920 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Top of safe zone:
"No other tool shows you this:" in color #F9E2AF (yellow), bold, center-aligned, medium-large font.

[40px gap]

Center section — a terminal window mockup:
- Rounded rectangle, background #313244, border #45475A
- Three dots in title bar: #F38BA8, #F9E2AF, #A6E3A1
- Inside the terminal, three rows of session data in monospace font, left-aligned with consistent column spacing:

  Row 1: "auth-refactor" in #CDD6F4, then "$0.47" in #A6E3A1 (green), then "12.3k tokens" in #89B4FA (blue)
  Row 2: "fix-ci-pipeline" in #CDD6F4, then "$1.23" in #A6E3A1 (green), then "31.2k tokens" in #89B4FA (blue)
  Row 3: "add-tests" in #CDD6F4, then "$0.08" in #A6E3A1 (green), then "2.1k tokens" in #89B4FA (blue)

The dollar amounts should be clearly legible and visually prominent.

[40px gap]

Below the terminal:
"Per-session cost tracking." in color #FAB387 (peach), bold, large.
"Know exactly what you're spending." in color #CDD6F4, medium.

Dark developer aesthetic. The terminal should look authentic. No emojis, no decorations beyond the terminal frame.
```

### Story Slide 5 — How It Works

```
Create a minimalist Instagram Story image at 1080x1920 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Three steps stacked vertically in the safe zone, center-aligned, with generous spacing (60px between steps):

Step 1: A circle with background #CBA6F7 (purple) containing "1" in #1E1E2E (dark), followed by "Pick your project" in #CDD6F4, medium monospace font.

Step 2: A circle with background #CBA6F7 containing "2" in #1E1E2E, followed by "Pick your session" in #CDD6F4.

Step 3: A circle with background #CBA6F7 containing "3" in #1E1E2E, followed by "You're back in context" in #A6E3A1 (green), slightly bolder to emphasize the payoff.

[60px gap]

Below the steps, a subtle divider line in #313244.

Three feature bullets below:
- Green checkmark (#A6E3A1) + "Full preview panel" in #CDD6F4
- Green checkmark (#A6E3A1) + "Fuzzy search everything" in #CDD6F4
- Green checkmark (#A6E3A1) + "Ctrl+D to delete" in #CDD6F4

Clean, structured layout. No images, no gradients. Developer aesthetic. No emojis.
```

### Story Slide 6 — The Comparison

```
Create a minimalist Instagram Story image at 1080x1920 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Three rows of text, center-aligned in the safe zone, with 48px vertical spacing:

Row 1: "Claude Squad" in #6C7086 (muted gray), followed by a right arrow "→" in #6C7086, followed by "6.6k stars, Go, heavy" in #6C7086. This row should look faded/deemphasized.

Row 2: "claude-history" in #6C7086, followed by "→" in #6C7086, followed by "Rust binary, no costs" in #6C7086. Also faded/deemphasized.

Row 3: "claude-picker" in #A6E3A1 (bright green), followed by "→" in #A6E3A1, followed by "432 lines, shows costs," on the first line and "Unix philosophy" on the second line, both in #A6E3A1. This row should have a subtle glow or highlight — perhaps a faint #A6E3A1 background glow or a thin left border bar in #A6E3A1.

[80px gap]

At the bottom of the safe zone:
"Sometimes less is more." in #CBA6F7 (purple), slightly larger, center-aligned.

The visual hierarchy must make the claude-picker row clearly stand out while the competitors fade into the background. No emojis.
```

### Story Slide 7 — CTA

```
Create a minimalist Instagram Story image at 1080x1920 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Center-aligned layout in the safe zone.

Top section:
A simplified GitHub logo (the octocat silhouette) rendered as a white (#CDD6F4) outline icon, approximately 80x80 pixels, centered.

[40px gap]

"MIT licensed. Works now." in #CDD6F4, medium font, center-aligned.

[30px gap]

"github.com/anshul-garg27/claude-picker" in #89B4FA (blue), styled to look like a clickable URL — perhaps with a subtle underline in #89B4FA. This should be the most prominent text on the slide.

[50px gap]

"Star it. Try it. Tell me what breaks." in #F9E2AF (yellow), slightly playful and casual tone, center-aligned.

Clean, minimal, inviting. The blue URL should draw the eye. No busy decorations. No emojis.
```

---

## LinkedIn Carousel Prompts (1080 x 1350 px)

### Carousel Slide 1 — Cover

```
Create a professional LinkedIn carousel slide image at 1080x1350 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Center-aligned text with generous padding (100px all sides).

Three lines of text, stacked with 36px line spacing:

Line 1: "I have 47 Claude Code sessions." in #CDD6F4, clean sans-serif font, medium-large size. The "47" should be in #F9E2AF (golden yellow) and bolder.

Line 2: "I can't find anything." in #CDD6F4, same size.

Line 3: "So I built a fix." in #A6E3A1 (green), slightly larger and bolder than the lines above.

Bottom right corner: "anshul-garg27" in #6C7086 (muted gray), small text.

Top right corner: "1/10" slide counter in #6C7086, very small.

Minimalist. No images, no icons, no decorations. Just text on dark. The negative space is intentional. Professional but not corporate. No emojis.
```

### Carousel Slide 2 — The Problem

```
Create a professional LinkedIn carousel slide image at 1080x1350 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Top section (within 100px padding):
"The Claude Code session problem" in #CBA6F7 (purple), bold, large sans-serif font, left-aligned.

[40px gap]

Five bullet points, left-aligned, with #FAB387 (peach) dash markers and text in #CDD6F4, monospace font, comfortable line spacing (32px):

- "Unnamed conversations everywhere"
- "No way to preview before resuming"
- "Re-explaining context you already gave"
- "No idea what each session cost you"
- "Can't search across conversations"

[50px gap]

Bottom section:
"Sound familiar?" in #F9E2AF (yellow), slightly larger, left-aligned.

Slide counter "2/10" in top right, #6C7086. Clean developer aesthetic. No emojis.
```

### Carousel Slide 3 — Solution

```
Create a professional LinkedIn carousel slide image at 1080x1350 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Center-aligned layout within 100px padding.

Top half:
"claude-picker" in large #CBA6F7 (purple), bold monospace font, centered — this is the hero text.

Below (20px gap):
"Browse, preview, and resume" in #A6E3A1 (green), medium font.
"Claude Code sessions from your terminal." in #CDD6F4, same size.

[50px gap]

Center: A terminal window mockup.
- Rounded rectangle, background #313244, subtle border #45475A
- Three dots in title bar: #F38BA8 (red), #F9E2AF (yellow), #A6E3A1 (green)
- Inside: "$ claude-picker" in #CDD6F4 monospace, cursor block in #CBA6F7 (purple)
- Terminal should be approximately 800px wide and 150px tall

[40px gap]

Bottom:
"bash + python + fzf" in #CDD6F4, small.
"432 lines. MIT licensed." in #F9E2AF (yellow), small.

Slide counter "3/10" in top right, #6C7086. No emojis.
```

### Carousel Slide 4 — Step 1

```
Create a professional LinkedIn carousel slide image at 1080x1350 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Top section (100px padding):
"Step 1: Pick your project" in #CBA6F7 (purple), bold, left-aligned.

[30px gap]

"Two-step fzf picker." in #CDD6F4, left-aligned.
"First, you see all your projects." in #CDD6F4.
"Fuzzy search to filter instantly." in #A6E3A1 (green).

[40px gap]

Center: Terminal mockup (800x350px approximately):
- Rounded rectangle, background #313244, border #45475A
- Three title bar dots
- Inside, an fzf-style list:
  Line 1: "> my-api" — the ">" in #A6E3A1 (green), "my-api" in #CDD6F4 with a subtle highlight bar (#45475A background)
  Line 2: "  frontend-app" in #6C7086 (muted)
  Line 3: "  infra-scripts" in #6C7086
  Line 4: "  docs-site" in #6C7086

The selected line should clearly stand out from the others.

Slide counter "4/10" in top right, #6C7086. Developer-authentic aesthetic. No emojis.
```

### Carousel Slide 5 — Step 2

```
Create a professional LinkedIn carousel slide image at 1080x1350 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Top section:
"Step 2: Pick your session" in #CBA6F7 (purple), bold, left-aligned.

[20px gap]

"Every session shows:" in #CDD6F4.
Four small bullets in #6C7086: "Conversation preview", "Token count + cost", "Last modified time", "Auto-generated name"

[30px gap]

Terminal mockup (800x300px):
- Rounded rectangle, #313244 background, #45475A border
- Three title bar dots
- Inside, three rows of fzf-style session data in monospace:

Row 1 (selected): "> fix-auth-flow" in #CDD6F4 on #45475A highlight bar, "$0.47" in #A6E3A1 (green), "12.3k tok" in #89B4FA (blue)
Row 2: "  add-rate-limiting" in #6C7086, "$1.23" in #A6E3A1, "31.2k tok" in #89B4FA
Row 3: "  refactor-db-layer" in #6C7086, "$0.08" in #A6E3A1, "2.1k tok" in #89B4FA

A thin vertical line in #45475A on the right third of the terminal suggests the preview panel area.

Slide counter "5/10" in top right, #6C7086. No emojis.
```

### Carousel Slide 6 — Killer Feature

```
Create a professional LinkedIn carousel slide image at 1080x1350 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

THIS IS THE MOST IMPORTANT SLIDE — make it visually striking.

Top section:
"Per-session cost tracking" in #FAB387 (peach/orange), large bold font, center-aligned. This should be the largest text on any slide in the carousel.

Below:
"No other Claude tool does this." in #F9E2AF (yellow), medium font, center-aligned, bold.

[40px gap]

Terminal mockup (800x320px), centered:
- Background #313244, border #45475A, three title bar dots
- Four rows of session data in monospace:

"auth-refactor" #CDD6F4 | "$0.47" #A6E3A1 | "12.3k tokens" #89B4FA
"fix-ci-pipeline" #CDD6F4 | "$1.23" #A6E3A1 | "31.2k tokens" #89B4FA
"add-tests" #CDD6F4 | "$0.08" #A6E3A1 | "2.1k tokens" #89B4FA
"debug-websockets" #CDD6F4 | "$2.87" #A6E3A1 | "72.4k tokens" #89B4FA

The dollar column should have a subtle green glow or be slightly larger than other columns.

[40px gap]

Bottom:
"Stop guessing. Start knowing." in #A6E3A1 (green), bold, center-aligned.

Slide counter "6/10" in top right, #6C7086. No emojis.
```

### Carousel Slide 7 — Feature List

```
Create a professional LinkedIn carousel slide image at 1080x1350 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Top:
"Everything it does" in #CBA6F7 (purple), bold, left-aligned.

[30px gap]

Ten features listed vertically with green (#A6E3A1) checkmark symbols, text in #CDD6F4, monospace font, 28px line spacing:

checkmark "Two-step fzf picker (project then session)"
checkmark "Live conversation preview panel"
checkmark "Named sessions (claude --name 'feature-x')"
checkmark "Full-text content search"
checkmark "Per-session token and cost display"
checkmark "Auto-naming for unnamed sessions"
checkmark "Ctrl+D to delete sessions"
checkmark "ANSI 256-color terminal UI"
checkmark "Fuzzy search across everything"
checkmark "Works in any terminal + Warp"

The list should fill the available space comfortably without feeling cramped. Left-aligned, 100px left padding.

Slide counter "7/10" in top right, #6C7086. No emojis.
```

### Carousel Slide 8 — Comparison

```
Create a professional LinkedIn carousel slide image at 1080x1350 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Top:
"How it compares" in #CBA6F7 (purple), bold, left-aligned.

[30px gap]

A comparison table with three columns and five rows, styled cleanly:

Header row: blank | "claude-picker" in #A6E3A1 | "Claude Squad" in #6C7086 | "claude-history" in #6C7086

Row 1 label "Cost tracking" in #CDD6F4:
  claude-picker: "YES" in bold #A6E3A1
  Claude Squad: "No" in #6C7086
  claude-history: "No" in #6C7086

Row 2 label "Size" in #CDD6F4:
  claude-picker: "432 lines" in #A6E3A1
  Claude Squad: "Large (Go)" in #6C7086
  claude-history: "Medium (Rust)" in #6C7086

Row 3 label "Philosophy" in #CDD6F4:
  claude-picker: "Unix pipes" in #A6E3A1
  Claude Squad: "Standalone" in #6C7086
  claude-history: "Standalone" in #6C7086

Row 4 label "Install" in #CDD6F4:
  claude-picker: "curl + chmod" in #A6E3A1
  Claude Squad: "go install" in #6C7086
  claude-history: "cargo install" in #6C7086

The claude-picker column should have a subtle vertical highlight stripe or left border in #A6E3A1 to visually differentiate it. Table should use monospace font. Clean grid lines in #313244.

Slide counter "8/10" in top right, #6C7086. No emojis.
```

### Carousel Slide 9 — Philosophy

```
Create a professional LinkedIn carousel slide image at 1080x1350 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Center-aligned layout with generous padding (120px all sides).

Top:
"432 lines of bash + python + fzf" in #F9E2AF (yellow), bold, center-aligned, large font.

[50px gap]

Center section, three groups of text with 40px between groups:

Group 1:
"No compiled binary." in #CDD6F4
"No build step." in #CDD6F4
"No runtime dependency you don't already have." in #CDD6F4

Group 2:
"Read the source in 10 minutes." in #CDD6F4
"Modify it for your workflow." in #CDD6F4
"Pipe it into whatever you want." in #CDD6F4

[50px gap]

Bottom section:
A green (#A6E3A1) ">" terminal prompt character, followed by:
"This is how Unix tools should work." in #CBA6F7 (purple), bold, larger than body text. This is the manifesto line and should be the visual anchor of the slide.

Slide counter "9/10" in top right, #6C7086. No emojis.
```

### Carousel Slide 10 — CTA

```
Create a professional LinkedIn carousel slide image at 1080x1350 pixels.

Background: solid #1E1E2E (Catppuccin Mocha Base).

Center-aligned layout within 100px padding.

Top:
"Try it now" in #A6E3A1 (green), large bold font, centered.

[30px gap]

Simplified GitHub octocat logo as white (#CDD6F4) outline, 60x60px, centered.

[20px gap]

"github.com/anshul-garg27/claude-picker" in #89B4FA (blue), medium monospace font, center-aligned. Subtle underline in #89B4FA to suggest a hyperlink.

[30px gap]

"MIT licensed. Works today." in #CDD6F4, center-aligned.

[40px gap]

Three action items, center-aligned, with colored bullet dots:
- Yellow dot (#F9E2AF): "Star it if it's useful." in #CDD6F4
- Peach dot (#FAB387): "Open an issue if it breaks." in #CDD6F4
- Purple dot (#CBA6F7): "Fork it if you want more." in #CDD6F4

[50px gap]

Bottom:
"Built by @anshul-garg27" in #6C7086 (muted gray), small.

Slide counter "10/10" in top right, #6C7086. No emojis.
```

---

# CATPPUCCIN MOCHA COLOR REFERENCE

For quick reference when creating any additional assets:

| Name | Hex | Usage |
|------|-----|-------|
| Base (Background) | #1E1E2E | All backgrounds |
| Text | #CDD6F4 | Primary body text |
| Subtext 0 | #6C7086 | Muted/secondary text, deemphasized elements |
| Surface 0 | #313244 | Terminal backgrounds, cards, containers |
| Surface 1 | #45475A | Borders, dividers, highlight bars |
| Lavender/Purple | #CBA6F7 | Headlines, brand color, claude-picker name |
| Green | #A6E3A1 | Success, costs, checkmarks, CTAs |
| Yellow | #F9E2AF | Emphasis, stats, numbers, warnings |
| Blue | #89B4FA | Links, token counts, URLs |
| Peach | #FAB387 | Secondary emphasis, alert, cost tracking label |
| Red | #F38BA8 | Terminal title bar dot (used sparingly) |

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

- [ ] Day 2: Post a short "behind the build" story (why you chose bash over Go/Rust)
- [ ] Day 3: Share a specific use case with a screen recording
- [ ] Day 5: Post metrics update ("X stars in 3 days — here's what people asked for")
- [ ] Day 7: Share a feature request poll or "what should I build next"

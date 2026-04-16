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
**Voice**: First person, personal. A developer sharing what they built and why. Not marketing. Not "you have 47 sessions" — "I had 47 sessions". Feels like texting a friend.

Post all 10 stories in one sitting. They form a chronological story: *I was building something → I hit a problem → I got curious → I dug in → I built a fix → I kept extending it → I use it daily now → here it is.*

---

## Story 1 of 10 — Where it started

**Text on screen:**
```
been using claude code every day
for like six months now.

multiple projects.
way too many conversations.
```

**Visual direction:** Dark background (#1E1E2E). Casual, handwritten-feel. All lowercase to match the conversational tone. Centered, generous vertical breathing room. Feels like an iMessage on the lock screen.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Content centered in safe zone, avoiding top 250px and bottom 280px. 80px horizontal padding.

Four lines of text, all lowercase, conversational, stacked with 52px line spacing, center-aligned:

Line 1: "been using claude code every day" in #CDD6F4, clean sans-serif (not monospace — this is the personal intro slide), 48px.
Line 2: "for like six months now." in #CDD6F4, 48px.
[60px gap — deliberate pause]
Line 3: "multiple projects." in #CDD6F4, 44px.
Line 4: "way too many conversations." in #F9E2AF (yellow), 44px — subtle emphasis on the problem starting to show.

No images, decorations, or emojis. The feel is a personal journal entry, not a marketing hook. Plenty of negative space. Dark and intimate.
```

---

## Story 2 of 10 — The moment I hit the wall

**Text on screen:**
```
last week i needed to get back
to this one conversation i had
about auth middleware.

went to `claude --resume`

got this:
```

**Visual direction:** Text setup on the left, leaving room for a small terminal snippet at the bottom showing the ugly UUID list. Feels like I'm showing you my actual screen. The word "this" in the last line hangs — it sets up the next slide visually, but this one stops there for tension.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Left-aligned text block, 80px left padding, starts at 25% from top.

Lines with 44px line height, all lowercase:
"last week i needed to get back" in #CDD6F4, 42px.
"to this one conversation i had" in #CDD6F4, 42px.
"about auth middleware." in #CDD6F4, 42px.
[40px gap]
"went to " in #CDD6F4, 40px + "claude --resume" in monospace #89B4FA (blue), 40px, all on one line.
[40px gap]
"got this:" in #CDD6F4, 44px, slightly emphasized.

Below the text (starts at about 60% from top), an actual mini terminal mockup: 900x520px, background #181825, subtle border #313244, no title bar dots. Inside, seven rows of monospace 26px:

"? Pick a conversation to resume" in #6C7086
"4a2e8f1c-9b3d-4e7a-a891..." in #6C7086
"b7c9d2e0-1f4a-8b6c-d5e9..." in #6C7086
"e5f8a3b1-7c2d-9e0f-b4a5..." in #6C7086
"c2d6e1f7-3b9a-5c4d-e8f1..." in #6C7086
"a9b3c7d2-8e4f-1a6b-c5d9..." in #6C7086
"..." in #6C7086

All UUIDs in dim gray. Nothing highlighted. The ugliness IS the point. No emojis.
```

---

## Story 3 of 10 — Four wrong clicks later

**Text on screen:**
```
uuids and timestamps.
that's it.

i clicked through four wrong
conversations before finding
the right one.

and it's not the first time.
```

**Visual direction:** Left-aligned, conversational. "four" in yellow to emphasize the actual count. Minor detail: the word "wrong" is also subtly highlighted because it's the specific frustration. Feels like me venting.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Left-aligned text block, 80px left padding, vertically centered in safe zone.

All sentence-case, conversational tone, 44px line spacing:

"uuids and timestamps." in #CDD6F4, 46px.
"that's it." in #6C7086 (muted gray), 40px — resigned tone.
[50px gap]
"i clicked through " in #CDD6F4 + "four wrong" in #F9E2AF (yellow) + " " in #CDD6F4, 44px — "four wrong" emphasized.
"conversations before finding" in #CDD6F4, 44px.
"the right one." in #CDD6F4, 44px.
[50px gap]
"and it's not the first time." in #FAB387 (peach), 42px, slightly italic feel.

No terminal mockup here — this slide is pure text, pure venting. Dark, stark. No emojis.
```

---

## Story 4 of 10 — I got curious

**Text on screen:**
```
so instead of fixing
the auth thing i was
supposed to fix...

i opened ~/.claude/
to see what's in there.
```

**Visual direction:** Self-aware developer humor. The "instead of fixing" setup lands as a joke because developers know this exact feeling of getting nerd-sniped. The path `~/.claude/` is in monospace to signal "we're going into the terminal now".

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Left-aligned text, 80px left padding, vertically centered.

All lowercase, 44px line spacing, conversational:

"so instead of fixing" in #CDD6F4, 46px.
"the auth thing i was" in #CDD6F4, 46px.
"supposed to fix..." in #6C7086 (muted gray), 46px — the ellipsis and gray feel self-aware.
[60px gap]
"i opened " in #CDD6F4 + "~/.claude/" in monospace #A6E3A1 (green), 46px, all on one line.
"to see what's in there." in #CDD6F4, 46px.

No terminal mockup. Just text. Personal, curious, slightly guilty. No emojis.
```

---

## Story 5 of 10 — What I found

**Text on screen:**
```
turns out claude stores every
session as a jsonl file.

~/.claude/projects/<encoded-path>/
  abc123.jsonl
  def456.jsonl
  ghi789.jsonl
  ...

one file per conversation.
everything's in there.
```

**Visual direction:** The "aha" moment. Mini terminal tree structure shown clearly. Makes the technical discovery feel accessible. The ellipsis `...` lets viewers feel there's more.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Left-aligned text, 80px padding.

Top section, sentence-case conversational:
"turns out claude stores every" in #CDD6F4, 44px.
"session as a jsonl file." in #CDD6F4, 44px.
[60px gap]

Middle — small file tree in monospace 32px, background #181825 rounded rectangle with 40px padding, 900px wide:

"~/.claude/projects/<encoded-path>/" in #89B4FA (blue)
"  abc123.jsonl" in #CDD6F4
"  def456.jsonl" in #CDD6F4
"  ghi789.jsonl" in #CDD6F4
"  ..." in #6C7086

[50px gap]
"one file per conversation." in #CDD6F4, 42px.
"everything's in there." in #A6E3A1 (green), 42px — the discovery moment.

No emojis. The file tree should look like an actual `tree` command output.
```

---

## Story 6 of 10 — Started building

**Text on screen:**
```
so i started writing
something to read them.

two hours later i had
a working session picker.

bash + python + fzf.
no dependencies i didn't
already have.
```

**Visual direction:** The casual reveal of what I built. No hype. "Two hours later" signals this wasn't a massive engineering effort — it was a Saturday afternoon thing. Makes it feel approachable.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Left-aligned, 80px padding, vertically centered in safe zone.

Conversational, lowercase, 44px line spacing:

"so i started writing" in #CDD6F4, 44px.
"something to read them." in #CDD6F4, 44px.
[50px gap]
"two hours later i had" in #CDD6F4, 44px.
"a working session picker." in #A6E3A1 (green), 44px.
[60px gap]
"bash + python + fzf." in monospace #F9E2AF (yellow), 40px.
"no dependencies i didn't" in #CDD6F4, 40px.
"already have." in #CDD6F4, 40px.

No terminal mockup, no decorations. The words carry the slide. No emojis.
```

---

## Story 7 of 10 — Then I kept adding stuff

**Text on screen:**
```
then i was curious
how much claude was
actually costing me.

added --stats.

  total:    $38.27
  architex  $12.40
  infra     $5.13
  ...

turns out i was spending
more on one project
than on lunch.
```

**Visual direction:** The "kept adding stuff" moment — shows the natural progression. The cost numbers are specific and slightly uncomfortable (especially "more than on lunch" — a relatable self-roast). Not bragging about features, just telling what happened next.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Left-aligned, 80px padding.

Top text, conversational lowercase, 44px line spacing:
"then i was curious" in #CDD6F4, 42px.
"how much claude was" in #CDD6F4, 42px.
"actually costing me." in #CDD6F4, 42px.
[40px gap]
"added " in #CDD6F4 + "--stats." in monospace #F9E2AF (yellow), 44px, on one line.

[40px gap]
Mini terminal box 800x280px, background #181825, 40px padding, monospace 28px:
"  total:    $38.27" — label #CDD6F4, amount #A6E3A1 (green)
"  architex  $12.40" — same coloring
"  infra     $5.13"  — same
"  ..."              — in #6C7086

[40px gap]
"turns out i was spending" in #CDD6F4, 40px.
"more on one project" in #CDD6F4, 40px.
"than on lunch." in #FAB387 (peach), 40px, slightly italic — self-roast tone.

No emojis.
```

---

## Story 8 of 10 — Search, because memory is bad

**Text on screen:**
```
kept trying to remember
stuff like "where did i fix
that race condition"

so added --search.

every message. every session.
every project.

grep, but for conversations.
```

**Visual direction:** The honesty about forgetfulness makes this relatable. The phrase "grep, but for conversations" is the money quote — developers get it instantly.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Left-aligned, 80px padding, vertically centered.

Conversational lowercase, 44px line spacing:

"kept trying to remember" in #CDD6F4, 42px.
"stuff like " in #CDD6F4 + "\"where did i fix" — opening quote and text in #A6E3A1 (green), 42px, on one line.
"that race condition\"" in #A6E3A1 (green), 42px — closed quote same color.
[40px gap]
"so added " in #CDD6F4 + "--search." in monospace #F9E2AF (yellow), 44px, on one line.

[50px gap]
"every message. every session." in #CDD6F4, 40px.
"every project." in #CDD6F4, 40px.

[50px gap]
"grep, but for conversations." in #CBA6F7 (purple), 44px, bold — the money line.

No emojis. Minimal, punchy.
```

---

## Story 9 of 10 — How I actually use it now

**Text on screen:**
```
six months later, i use
this thing like 20 times a day.

also started naming every
claude session:

  claude --name "auth-refactor"
  claude --name "drizzle-fix"

takes 2 seconds.
saves me like 10 minutes.
```

**Visual direction:** The real outcome. Not "it changed my life" — just "this is what I do now". The `claude --name` examples are in monospace because they're actual commands you can copy. The final two lines are the honest math.

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Left-aligned, 80px padding.

Conversational lowercase, 44px line spacing:

"six months later, i use" in #CDD6F4, 42px.
"this thing like " in #CDD6F4 + "20 times a day" in #F9E2AF (yellow), 42px, on one line + "." at end in #CDD6F4.
[50px gap]
"also started naming every" in #CDD6F4, 42px.
"claude session:" in #CDD6F4, 42px.
[40px gap]

Mini command block in monospace 30px, 40px left-indented:
"claude --name \"auth-refactor\"" — "claude" in #CDD6F4, "--name" in #89B4FA, string in #A6E3A1
"claude --name \"drizzle-fix\""    — same coloring

[50px gap]
"takes 2 seconds." in #CDD6F4, 40px.
"saves me like 10 minutes." in #A6E3A1 (green), 40px — the payoff.

No emojis. Honest, personal.
```

---

## Story 10 of 10 — Here if anyone wants it

**Text on screen:**
```
put it on github in case
anyone else has this problem.

github.com/anshul-garg27/claude-picker

mit licensed.
bash + python + fzf.
works in any terminal.

if you try it, lmk what breaks.
```

**Visual direction:** Low-key CTA. Not "STAR MY REPO!" — just "here's the link, let me know if it breaks". The humility is the hook. The URL should be the most prominent element because that's the only thing people need.

**Instagram link sticker:** github.com/anshul-garg27/claude-picker

**Gemini AI Pro prompt:**
```
Create a minimalist Instagram Story image at 1080x1920 pixels. Background: solid #1E1E2E (Catppuccin Mocha Base). Left-aligned, 80px padding. Text vertically centered in safe zone.

Conversational lowercase, 44px line spacing:

"put it on github in case" in #CDD6F4, 42px.
"anyone else has this problem." in #CDD6F4, 42px.
[50px gap]

The URL prominently: "github.com/anshul-garg27/claude-picker" in monospace #89B4FA (blue), 42px, with subtle #89B4FA underline to suggest it's clickable. This is the most visually dominant element on the slide.

[50px gap]
Three small facts, each on its own line, 32px monospace #6C7086 (muted gray):
"mit licensed."
"bash + python + fzf."
"works in any terminal."

[50px gap]
"if you try it, lmk what breaks." in #F9E2AF (yellow), 38px — casual, honest sign-off.

No GitHub logo icon (would feel corporate). Keep it text-only. No emojis. The link sticker handles the actual tap target.
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

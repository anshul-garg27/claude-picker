# claude-picker Launch Playbook

Based on deep research of 50+ sources, competitor analysis, and successful open-source launch patterns.

---

## Messaging Matrix

Which message hits which audience. Each platform rewards a different tone — don't cross-post the same copy.

| Platform | Audience mindset | Tone | Lead with | Avoid |
|----------|------------------|------|-----------|-------|
| **Hacker News** | Technical, skeptical, reads source | Understated, specific, honest tradeoffs | Technical details: ~800 lines, fzf 0.58+, Rich, how `--search` indexes JSONL files, why bash over Rust | Marketing language, "we", hype, unsupported superlatives |
| **Reddit (r/ClaudeCode, r/ClaudeAI)** | Daily Claude users, pain-point focused | Personal, slightly self-deprecating, story-driven | The frustration ("I had 47 sessions and couldn't find anything"), the fix, a GIF | Corporate tone, feature dumps, "check out my project" |
| **Reddit (r/commandline)** | fzf/tmux/vim crowd, aesthetic-sensitive | Technical, Unix-philosophy | Two-step fzf picker, keybindings, pipeable output (`--pipe`), how it reads `~/.claude/` | Claude Code-specific framing (they care about the tool, not the model) |
| **Product Hunt** | Makers, designers, early adopters | Polished, confident, benefit-first | The visual hook, the tagline, screenshots of `--stats` dashboard | Raw terminal dumps, insider jargon, long paragraphs |
| **LinkedIn** | Professional devs, managers, recruiters | Credible, problem-solution | Problem framing, personal angle ("I built"), specific numbers | Low-effort memes, informal slang, bash one-liners without context |
| **Twitter/X** | Fast scroll, visual, DevRel audience | Punchy, thread format, video-first | The demo video, one-line hook, per-tweet single-feature | Walls of text, no media, generic "launch day" copy |
| **Warp community / Discord** | Power-users of specific tools | Casual, tool-native | Warp `+` menu integration, `/claude-picker` slash command, keybinding workflow | Pitch decks, broad positioning |
| **Dev.to / Hashnode** | Long-form readers, tutorial-seekers | Walkthrough, code snippets, architecture | Full article: "Why I built this", design decisions, how the preview panel works, Rich formatting details | Pure marketing, missing technical depth |
| **Newsletters (TLDR, Console.dev)** | Editor-filtered, broad reach | Tight 2-sentence pitch, one link | The single most unique feature (cost per session + stats combined), the ~800 lines | Multiple links, jargon, unclear what it does |

---

## Positioning

> **claude-picker**: Find any Claude Code conversation by content, see what it cost you, compare two sessions side-by-side, and resume — in one keystroke. The Unix way.

**One-liner variants for different contexts:**

- **HN title**: `Show HN: claude-picker – Browse, search, and resume Claude Code sessions with fzf`
- **Reddit title**: `I built a session manager for Claude Code that shows per-session cost and lets you search across every conversation`
- **Twitter hook**: `47 Claude Code sessions. Couldn't find the auth one. So I built claude-picker.`
- **LinkedIn hook**: `I have 47 Claude Code sessions. I can't find anything. So I built a fix.`
- **Product Hunt tagline**: `The session manager Claude Code forgot to ship`

**Unique angle vs competitors:** No other Claude session tool has this combination: **per-session cost + stats dashboard + full-text search across projects + session tree + session diff + bookmarks + markdown export + in-place delete**. Everyone else picks one or two. claude-picker does the lot, in ~800 lines of bash+python.

---

## Competitive Positioning

### Full feature comparison

| Feature | claude-picker | claude-history (Rust) | Claude Squad (Go) | ccmanager |
|---------|--------------|----------------------|-------------------|-----------|
| Per-session cost display | YES | No | No | No |
| Full-text search across all projects | YES | Partial | No | No |
| Stats dashboard (`--stats`) | YES | No | No | No |
| Session tree with fork detection (`--tree`) | YES | No | No | No |
| Session diff (`--diff`) | YES | No | No | No |
| Bookmarks (`Ctrl+B`) | YES | No | No | No |
| Markdown export (`Ctrl+E`) | YES | No | No | No |
| In-place delete (`Ctrl+D`) | YES | No | No | No |
| Two-step picker (project → session) | YES | No | No | No |
| Auto-named sessions from first message | YES | No | No | No |
| Age warnings (7d peach, 30d red) | YES | No | No | No |
| Git branch display per project | YES | No | No | No |
| `--pipe` for scripting | YES | No | No | No |
| Warp `+` menu integration | YES | No | No | No |
| `/claude-picker` Claude Code slash command | YES | No | No | No |
| Weight | ~800 lines | Compiled Rust | Compiled Go | — |
| Install | curl + chmod | cargo install | go install | npm |
| Category | Session picker | History viewer | Multi-agent orchestrator | Multi-agent |

### The moat

Five things no one else does together:

1. **Cost per session.** Nobody tracks this. You have no idea what you've spent.
2. **Stats dashboard.** Per-project breakdown, activity timeline, top 5 sessions by usage — all in `--stats`.
3. **Session tree.** Detects `forkedFrom` and draws parent-child relationships.
4. **Session diff.** Pick two, compare topics and conversations side-by-side.
5. **Global full-text search.** Every message in every session in every project. Hit enter, auto-cd into the project dir, resume.

Add bookmarks, markdown export, and in-place delete — it's the most feature-rich tool in the category, and it's ~800 lines.

---

## Pre-Launch (1-2 weeks before)

- [ ] Join and participate in r/ClaudeAI, r/ClaudeCode, r/commandline (build karma)
- [ ] Comment helpfully on Claude Code session management threads (link #8701, #11408, #6907)
- [ ] Seed 100-200 GitHub stars from personal network (credibility floor)
- [ ] Record all demo GIFs: `for t in scripts/tapes/*.tape; do vhs "$t"; done`. Output lands in `assets/gifs/` (hero, search, stats, tree, diff, bookmarks, export) and `assets/videos/`. Ensure `hero.gif` stays under 4 MB for Twitter — trim with `ffmpeg -i assets/gifs/hero.gif -vf "fps=12,scale=800:-1:flags=lanczos" -loop 0 hero-twitter.gif` if needed.
- [ ] Generate static mockups: `bash assets/generate.sh` (writes `assets/mockups/*.png` and `*.svg`).
- [ ] Extract key frames: `bash assets/extract-frames.sh` (writes `assets/frames/*.png`).
- [ ] Generate AI images via Gemini AI Pro using prompts in `content/image-prompts.md` (25 prompts) and `content/instagram-linkedin.md` (10 IG stories + 12 LinkedIn slides). Save under `assets/ai-generated/<platform>/` per `content/USAGE.md`.
- [ ] Finalize README — `assets/gifs/hero.gif` above the fold, feature matrix, install one-liner.
- [ ] Pre-write ALL posts (HN, Reddit x3, Dev.to, Twitter, LinkedIn, PH)
- [ ] Prepare newsletter pitch emails (TLDR, Console.dev, Changelog)
- [ ] Verify install works on fresh macOS + fresh Ubuntu (smoke test)
- [ ] Draft 5 FAQ reply comments ready to paste (Rust-rewrite question, Windows question, why fzf, why not Go, what about claude-history)

---

## Product Hunt Launch

### Timeline (launch at 12:01 AM PT on Tuesday/Wednesday)

| Time (PT) | Action |
|-----------|--------|
| **12:01 AM** | PH listing goes live (scheduled the night before). First maker comment within 60 seconds. |
| **12:05 AM** | Notify 5-10 trusted hunter friends via Discord/WhatsApp (do NOT mass-ping) |
| **12:30 AM** | Post in Warp Discord, Claude Code Discord, relevant Slack communities |
| **6:00 AM** | Wake up. First comment reply sweep. Check rank. |
| **7:00 AM** | HN Show HN post goes live (separate channel, separate title) |
| **8:00 AM** | Twitter thread goes live (6 tweets, terminal video) |
| **8:30 AM** | Reddit posts go live (r/ClaudeCode, r/ClaudeAI, r/commandline — stagger 15 min apart) |
| **9:00 AM** | LinkedIn carousel post |
| **9:30 AM** | Email TLDR, Console.dev, Changelog |
| **10:00 AM - 2:00 PM** | Reply to every PH comment within 10 minutes. Comments drive ranking. |
| **12:00 PM** | Lunchtime push — share in 2-3 more Discord communities. Post Dev.to article. |
| **3:00 PM** | Mid-afternoon comment sweep. Post "thank you + current rank" update in communities. |
| **6:00 PM** | Evening US + morning EU push. Reply to every comment. |
| **9:00 PM** | Wind-down. Post wrap-up tweet. Plan day-2 follow-up. |
| **11:45 PM** | Final comment sweep before voting closes. |

### Listing content

**Title** (60 char max):
```
claude-picker — session manager for Claude Code
```

**Tagline** (60 char max):
```
Browse, search, and resume Claude sessions in one keystroke
```

**Description** (260 char max):
```
The session manager Claude Code forgot to ship. Two-step fzf picker, per-session cost tracking, full-text search across every conversation, stats dashboard, session tree with forks, diff. ~800 lines of bash+python. MIT.
```

**First maker comment (post within 60 seconds of launch):**
```
Hey PH — Anshul here.

I built claude-picker because I had 47 Claude Code sessions across 6 projects and couldn't find anything.

Claude Code ships with `--resume` but no way to preview, search, or see cost. So I built a two-step fzf picker: pick your project, pick your session, you're back in context.

The features I'm most proud of:

• Per-session cost tracking. No other Claude tool shows this. You see "$0.47 / 12.3k tokens" right in the picker.
• `--stats` dashboard. Total sessions, total spend, per-project bars, activity timeline, top 5 sessions by usage.
• `--tree` detects fork relationships via the `forkedFrom` field and draws the parent-child tree.
• `--search` indexes every message across every session in every project. Select a match, auto-cd into the project, resume.
• `--diff` picks two sessions and compares topics and conversations side-by-side.
• Bookmarks, markdown export, in-place delete (Ctrl+B/E/D).

It's ~800 lines of bash + python3 + fzf 0.58+ + Rich. MIT. Read the source in 20 minutes.

Would love your honest feedback — specifically: what would you add? What's broken on your system? What's the workflow I missed?

Star it, break it, open issues:
github.com/anshul-garg27/claude-picker
```

### Image specs

Save every Product Hunt image under `assets/ai-generated/producthunt/`. See `content/USAGE.md` for the complete asset-to-platform map.

- **Gallery image 1** (1270x760 hero) → `gallery-01-hero.png`: Dark Catppuccin Mocha background, terminal mockup showing the two-step picker with cost column, tagline "The session manager Claude Code forgot to ship" in #CBA6F7. *Fast path: resize `assets/frames/hero-02-sessions.png` to 1270x760 instead of generating.*
- **Gallery image 2** (1270x760) → `gallery-02-stats.png`: `--stats` dashboard. *Fast path: resize `assets/mockups/stats.png`.*
- **Gallery image 3** (1270x760) → `gallery-03-tree.png`: `--tree` with fork relationships. *Fast path: `assets/mockups/tree.png`.*
- **Gallery image 4** (1270x760) → `gallery-04-search.png`: `--search` results across projects. *Fast path: `assets/frames/search-01-query.png`.*
- **Gallery image 5** (1270x760) → `gallery-05-diff.png`: `--diff` comparison. *Fast path: `assets/mockups/diff.png`.*
- **Gallery image 6** (1270x760) → `gallery-06-shortcuts.png`: In-picker shortcuts (Ctrl+B/E/D/P) with keycaps. (Generate via image-prompts.md.)
- **Thumbnail/logo** (240x240) → `thumbnail.png`: "cp" monogram in #CBA6F7 on #1E1E2E, monospace. (Generate via image-prompts.md prompt #20.)

All images use Catppuccin Mocha hex codes (see [color reference](#catppuccin-mocha-color-reference)). No emojis. Explicit safe zone: 80px padding all sides.

---

## Launch Day (Tuesday/Wednesday/Thursday) — Hour-by-Hour

| Time (ET) | Platform | Action | Notes |
|-----------|----------|--------|-------|
| **6:00 AM** | Coffee. Open 5 browser tabs: HN, PH, Reddit, Twitter, LinkedIn. | Do not post yet. |
| **7:00 AM** | **Hacker News** | Submit Show HN. Title: `Show HN: claude-picker – Browse, search, and resume Claude Code sessions with fzf`. First founder comment within 5 minutes (see HN comment template below). | First 30 minutes determine front-page fate. |
| **7:05 AM** | HN founder comment | Paste pre-written comment explaining the "why" and tradeoffs. | Be understated, not salesy. |
| **7:15 AM** | Monitor | HN rank at 7:15, 7:30, 7:45, 8:00. Reply to first 2-3 comments within 5 min. | Don't vote-beg. |
| **8:00 AM** | **Reddit r/ClaudeCode** | Personal story post with demo GIF. Lead with frustration, not features. | Use title: "I got tired of losing my Claude Code sessions so I built a session manager" |
| **8:15 AM** | **Reddit r/ClaudeAI** | Cross-post with slightly different angle (emphasize cost tracking) | Title: "claude-picker: the only Claude session tool that tracks cost per session" |
| **8:30 AM** | **Reddit r/commandline** | Technical fzf angle | Title: "A two-step fzf picker for Claude Code sessions — bash + python, ~800 lines" |
| **9:00 AM** | **Twitter/X thread** (6 tweets) | Tweet 1: Hook + GIF. Tweets 2-5: One feature per tweet with screenshot. Tweet 6: Link + "what should I build next?" | Use terminal video, not still images. Under 2:20 video. |
| **9:00 AM** | **Dev.to** | Full long-form article | "I built a session manager for Claude Code in bash+python (and why cost tracking matters)" |
| **9:30 AM** | **Product Hunt** | If PH launch day, make sure the morning boost lines up | Reply to every comment within 10 min |
| **10:00 AM** | **LinkedIn carousel** | 12-slide PDF post | Don't cross-post text post same day |
| **10:15 AM** | **Email TLDR** | submissions@tldr.tech — 2-sentence pitch | See template below |
| **10:30 AM** | **Email Console.dev** | console.dev/submit | See template below |
| **10:45 AM** | **Email Changelog** | changelog.com/submit | See template below |
| **11:00 AM** | **Discord push** | Charm.sh, Warp, Claude Code, AI Engineering communities | Casual tool-native framing |
| **12:00 PM** | **Hashnode** | Cross-post from Dev.to with canonical URL | SEO move |
| **12:30 PM** | **Lunch comment sweep** | Reply to every HN, Reddit, Twitter, LinkedIn, PH comment | |
| **2:00 PM** | **GitHub Issues** | Link the tool in Claude Code issues #8701, #11408, #6907, #47945 | Don't spam — just "I built this in case it's useful" |
| **3:00 PM** | **Second wave** | Post in smaller niche Slacks (AI tools, devtools, indie hackers) | |
| **5:00 PM** | **EU/morning US push** | Another comment sweep | |
| **7:00 PM** | **West coast push** | Final outreach, evening PH comment sweep | |
| **9:00 PM** | **Wrap tweet** | "Day 1: N stars, M issues opened, thanks everyone" | Sets up day-2 content |
| **11:00 PM** | **Final sweep** | Reply to everything before bed | |

---

## Day 2

- [ ] Respond to EVERY comment on HN, Reddit, Twitter, LinkedIn, PH
- [ ] "Building in public" follow-up post: "24 hours in — N stars, M issues, here's what people asked for most"
- [ ] Submit to awesome lists:
  - [ ] hesreallyhim/awesome-claude-code (use issue template)
  - [ ] rohitg00/awesome-claude-code-toolkit
  - [ ] travisvn/awesome-claude-skills
  - [ ] rosaboyle/awesome-cc-oss
  - [ ] awesome-cli-apps
  - [ ] awesome-shell
  - [ ] sindresorhus/awesome-fzf
- [ ] Follow-up newsletter pitches with "N stars in 24h" social proof
- [ ] Record a short Loom (3 min) walking through `--stats`, `--tree`, `--diff` for a deeper follow-up Reddit post
- [ ] Check Warp `+` menu — confirm tile submission for Warp-native distribution

---

## Day 3-7

- [ ] Continue responding to all comments (under 12 hour turnaround)
- [ ] Day 3: "Behind the build" post — why bash, why fzf, why not Rust. Link to Dev.to article.
- [ ] Day 5: Post a specific use-case demo (e.g., "how I used `--diff` to compare two auth-refactor forks")
- [ ] Day 7: "What I'm building next" poll — fold top 3 issue requests into a v2.0 teaser
- [ ] Send metrics update to newsletters that haven't covered yet

---

## Newsletter Pitch Templates

Keep each pitch under 3 sentences. Newsletters get hundreds of submissions daily — brevity wins.

### TLDR (submissions@tldr.tech)

**Subject:** `claude-picker — session manager for Claude Code with cost tracking, search, stats, diff`

```
Hi TLDR team,

I built claude-picker, a terminal session manager for Claude Code: two-step fzf picker, per-session cost tracking (the only tool that shows this), full-text search across every conversation in every project, plus --stats dashboard, --tree with fork detection, and --diff to compare two sessions. It's ~800 lines of bash + python3 + fzf + Rich, MIT licensed, and works with zero compile step.

Would be a fit for the Dev or AI sections:
github.com/anshul-garg27/claude-picker

Happy to answer any questions.
Anshul
```

### Console.dev (console.dev/submit)

**Subject:** `Tool submission: claude-picker`

```
Tool name: claude-picker
One-line: Session manager for Claude Code with cost tracking, full-text search, stats dashboard, tree view, and diff.
Why it matters: Claude Code ships with --resume but no way to preview, search, or see cost. claude-picker is the only Claude session tool that shows per-session $ and tokens, and the only one with a stats dashboard, session tree, and diff — all in ~800 lines of bash+python. No compile step.
Link: github.com/anshul-garg27/claude-picker
Maker: @anshul-garg27
```

### Changelog (changelog.com/submit)

**Subject:** `Show Changelog: claude-picker — session manager for Claude Code`

```
Hey Changelog,

claude-picker is a terminal session manager for Claude Code. It's a two-step fzf picker (project → session) with per-session cost display, a --stats dashboard, a --tree view that detects forks via the forkedFrom field, full-text --search across every conversation, and --diff for comparing two sessions. ~800 lines of bash + python3 + fzf 0.58+ + Rich. MIT licensed.

Would love a mention in Changelog News or the Shipit newsletter:
github.com/anshul-garg27/claude-picker

Anshul
```

### Hacker Newsletter / daily.dev

Auto-curated — both pull from HN front page and Dev.to top posts. No direct submission needed, but:

- Tag the Dev.to post with `#claude-code`, `#devtools`, `#opensource`, `#cli` to surface on daily.dev
- Make sure the HN Show HN hits at least 50 upvotes in first 3 hours to trigger Hacker Newsletter

---

## Hacker News Comment Template (post within 5 minutes of Show HN)

```
Author here.

I had 47 Claude Code sessions across 6 projects. Couldn't find anything. Claude Code's `--resume` is a flat list of session IDs with no preview, no search, no cost info. So I built a two-step fzf picker.

Design decisions worth explaining:

- Bash + python3 + fzf + Rich, ~800 lines. No compile step. The point was to be auditable — you can read the whole thing in 20 minutes. I considered Rust but rejected it because fzf is already the best TUI primitive and Python3 + Rich handles the preview rendering cleanly.

- Cost per session is computed from the token counts in the JSONL files using the published Claude pricing. It's an estimate, not a billing-grade number, but it's accurate enough to see which sessions were expensive.

- `--search` does a linear scan across every JSONL under `~/.claude/projects/` on invocation. No index. At 2.4M tokens across 142 sessions it runs in <200ms on my machine. If the corpus gets bigger I'll add a cache.

- `--tree` reads the `forkedFrom` field that Claude Code writes when you branch a session. I'm not sure how many people know this field exists.

- The filter reads the `entrypoint` field and only shows sessions created by the Claude CLI, so non-Claude JSONLs in the directory don't pollute the list.

Happy to answer anything. The comparison table in the README goes into it vs claude-history (Rust) and Claude Squad (Go) — different categories.

Repo: github.com/anshul-garg27/claude-picker
```

---

## Content Targets

| Platform | Format | Primary article location |
|----------|--------|------------------------|
| Dev.to | Full article | **PRIMARY** (auto-surfaces on daily.dev) |
| Medium | Cross-post to Level Up Coding or Bootcamp publication | SECONDARY |
| Hashnode | Cross-post with canonical URL to Dev.to | SECONDARY |
| lobste.rs | Show HN-style link post (Unix-philosophy angle) | TERTIARY |

---

## Reference: Open GitHub Issues That Validate This Tool

Link these in posts to show demand:

- `anthropics/claude-code#8701` — Search conversation history in --resume
- `anthropics/claude-code#35599` — Support --resume latest
- `anthropics/claude-code#23954` — Picker keyboard navigation broken
- `anthropics/claude-code#29052` — Configurable session limit in /resume
- `anthropics/claude-code#47945` — Search sessions by UUID
- `anthropics/claude-code#11408` — Add ability to name and organize sessions
- `anthropics/claude-code#6907` — Auto-generate session summaries
- `anthropics/claude-code#24207` — No disk space management
- `anthropics/claude-code#32631` — Conversation branching

Each issue = a feature claude-picker already ships. Screenshot and link them in the Dev.to article.

---

## Feature Roadmap

### v1.0 — Shipped
- [x] Two-step fzf picker (project → session)
- [x] Conversation preview panel (Rich-formatted)
- [x] Catppuccin Mocha 24-bit theme
- [x] Labeled borders on fzf 0.58+

### v1.1 — Shipped (was "The Wow Update")
- [x] Full-text content search across sessions (`--search`)
- [x] Per-session token count + cost estimate in picker
- [x] Auto-generated display names for unnamed sessions (first user message)
- [x] Smart filter: only Claude CLI sessions (reads `entrypoint` field)

### v1.2 — Shipped (was "Power User Features")
- [x] Git branch display per project
- [x] One-key export to markdown (`Ctrl+E`)
- [x] Fork tree visualization (`--tree` with `forkedFrom` detection)
- [x] Session counts + activity bars per project
- [x] `--stats` dashboard (totals, per-project, activity timeline, top 5)
- [x] `--diff` side-by-side session comparison
- [x] `--pipe` for scripting (outputs session ID)
- [x] Bookmarks (`Ctrl+B`) with blue pin, top of list
- [x] In-place delete (`Ctrl+D`)
- [x] Age warnings (peach after 7d, red after 30d)
- [x] Section headers ("── saved ──" / "── recent ──")
- [x] Shell keybinding integration (`Ctrl+P` launches from anywhere)
- [x] Warp `+` menu tab config
- [x] `/claude-picker` Claude Code slash command

### v2.0 — Planned
- [ ] **TUI native mode** — optional fallback when fzf isn't installed, using Python Textual or prompt_toolkit
- [ ] **Session merging** — pick two sessions, combine their messages in order, write a new JSONL
- [ ] **Export templates** — `--export-as github-issue`, `--export-as gist`, `--export-as blog-draft`, configurable via `~/.config/claude-picker/templates/`
- [ ] **Session tagging** — local sidecar file in `~/.config/claude-picker/tags.json`, multi-tag filter in picker
- [ ] **Session templates / quick-start** — `claude-picker --new from-template api-debug`
- [ ] **Disk usage view** — `--disk` shows JSONL sizes per project, batch compress/delete old sessions
- [ ] **Cross-machine sync** — optional git-backed sync of bookmarks, tags, templates
- [ ] **Session search with regex** — extend `--search` with `--regex` flag
- [ ] **Time-window filter** — `--since 7d`, `--since 2024-01-01`

---

## Newsletter Contacts

| Newsletter | Audience | How | Note |
|------------|----------|-----|------|
| TLDR Tech | 1.25M+ | submissions@tldr.tech | Bi-weekly editor review; best for mass reach |
| Console.dev | Dev tools focused | console.dev/submit | Requires JSON-style submission; highest signal |
| Changelog | Open source weekly | changelog.com/submit | Features can lead to podcast invite |
| Hacker Newsletter | 60K+ | Auto-curated from HN front page | Need 50+ upvotes in first 3h |
| daily.dev | Millions | Auto from Dev.to posts with right tags | Tag `#claude-code` `#cli` `#opensource` |
| Indie Hackers Today | Indie maker crowd | indiehackers.com — post in "Show IH" | Cross-posts benefit the PH launch |
| Mind the Product | PM-adjacent devs | No direct sub — get cited via LinkedIn | Only applies if PM tools take interest |
| Dev.to weekly digest | Auto-selected from top posts | Dev.to editors curate | Post before 9 AM ET Tuesday for best chance |

---

## Competitors Reference

| Tool | Language | Stars | Unique Feature | Our Advantage |
|------|----------|-------|---------------|---------------|
| claude-history | Rust | ~197 | Full-text search, Vim viewer | No compile step. We add: cost, stats, tree, diff, bookmarks, export |
| claude-sessions | TypeScript | — | Multi-agent, AI summaries | Lighter weight, fzf native, cost display |
| cc-sessions | Rust | — | Some fork visualization | No Rust dependency. We add: stats, diff, cost, full-text search, bookmarks |
| ccmanager | — | — | Multi-agent support | Different category — we're picker-first |
| Claude Squad | Go | 6.6k | Parallel orchestration | Different category — we complement, not compete |

---

## Catppuccin Mocha Color Reference

For all launch imagery (PH gallery, LinkedIn carousel, IG stories, Twitter media):

| Name | Hex | Usage |
|------|-----|-------|
| Base (Background) | #1E1E2E | All backgrounds |
| Text | #CDD6F4 | Primary body text |
| Subtext/Muted | #6C7086 | Muted text, deemphasized elements |
| Surface 0 | #313244 | Terminal backgrounds, cards |
| Surface 1 / Border | #45475A | Borders, highlight bars |
| Lavender/Purple (brand) | #CBA6F7 | Headlines, brand, claude-picker name |
| Green (success) | #A6E3A1 | Costs, checkmarks, CTAs |
| Yellow (emphasis) | #F9E2AF | Numbers, stats, command flags |
| Blue (links) | #89B4FA | Links, token counts, URLs, bookmark pin |
| Peach (secondary) | #FAB387 | Fork labels, age warnings (7d+), diff divergence |
| Red (warning) | #F38BA8 | Terminal title bar dot, age warnings (30d+) |

---

## Post-Launch Metrics to Track

| Metric | Target by Day 7 | Target by Day 30 |
|--------|----------------|------------------|
| GitHub stars | 500 | 2,000 |
| GitHub forks | 20 | 100 |
| HN points | 150+ | — |
| PH rank | Top 10 day | Featured |
| Dev.to reactions | 200+ | 500+ |
| Twitter impressions | 50k | 200k |
| LinkedIn impressions | 20k | 80k |
| Newsletter mentions | 1 | 3-5 |
| Issues opened | 10-20 | 40-60 |
| PRs received | 2-3 | 8-12 |

If Day 7 is below half of these targets, the positioning or channel mix needs a rework — not the product.

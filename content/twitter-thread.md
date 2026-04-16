# Twitter/X Thread

Post Tuesday-Thursday, 9-11 AM EST for maximum reach.
ATTACH THE HERO GIF TO TWEET 1. Without it, engagement drops 3-5x.

All asset paths below are relative to the project root. See `content/USAGE.md` for the full asset-to-platform map.

---

## Tweet 1 (Hook — MUST stand alone)

You know that thing where you have 15 Claude Code sessions and can't remember which one had the fix you need?

I got tired of clicking through UUIDs. So I built a session browser.

bash + python + fzf. Works in any terminal.

**ATTACH:** `assets/gifs/hero.gif` (the main 15-second flow: project → session → preview → resume)

---

## Tweet 2 (Problem — relatable)

The built-in `claude --resume` gives you this:

```
4a2e8f1c-9b3d... (2 hours ago)
b7c9d2e0-1f4a... (3 hours ago)
e5f8a3b1-7c2d... (yesterday)
```

No project names. No preview. No way to tell sessions apart.

I was clicking through 4-5 wrong sessions every time.

**ATTACH:** `assets/mockups/before.png` (the "before" mockup — raw `claude --resume` UUID list)

---

## Tweet 3 (Solution — show, don't tell)

claude-picker gives you this:

- Project picker (git branch + session counts + activity bars)
- Session browser (named sessions on top)
- Conversation preview before opening
- Fuzzy search with fzf
- Named sessions pin above UUIDs

Under 500ms to list 50+ sessions.

**ATTACH:** `assets/mockups/sessions.png` (session picker with preview panel side-by-side)

---

## Tweet 4 (The interesting technical bit)

The fun part: reverse-engineering Claude Code's session storage.

Sessions are JSONL files in ~/.claude/projects/

But the path encoding is lossy — both / and _ become -

So /my_project and /my/project encode to the same thing.

Had to write 3 fallback strategies to crack it.

---

## Tweet 5 (--search)

Then I added --search.

Full-text search across every message in every session across every project.

"Which session was that race condition I debugged two weeks ago?"

Type the phrase. Get ranked sessions. Opens in the right project dir automatically.

**ATTACH:** `assets/gifs/search.gif`

---

## Tweet 6 (--stats)

And --stats, because I was curious how much Claude I was actually using.

- Total sessions / tokens / estimated cost
- Per-project breakdown with bar charts
- Today vs this week vs older
- Top 5 sessions by token count

Token estimate = content_length / 4. Rough but in the right ballpark.

**ATTACH:** `assets/mockups/stats.png` (or `assets/gifs/stats.gif` if the dashboard movement reads better than the still)

---

## Tweet 7 (--tree + fork detection)

Claude Code has /branch and --fork-session that fork conversations.

Every forked session has a forkedFrom field in its JSONL.

claude-picker --tree walks that graph:

```
auth-refactor
├── auth-refactor-jwt-variant   (forked)
└── auth-refactor-sessions      (forked)
```

Found 3 forks I'd completely forgotten about.

**ATTACH:** `assets/mockups/tree.png`

---

## Tweet 8 (Keyboard shortcuts)

Inside the picker:

- Ctrl+B: bookmark (pins to very top with blue ■)
- Ctrl+E: export session to clean markdown
- Ctrl+D: delete + auto-refresh
- Ctrl+P: shell keybinding, opens picker from anywhere
- Just type: fuzzy filter

The installer sets up Ctrl+P for zsh and bash.

**ATTACH:** `assets/gifs/bookmarks.gif` (Ctrl+B in action)

---

## Tweet 9 (--diff)

--diff picks two sessions and shows them side by side:

- Common topics
- Unique topics per session
- Conversation previews from both

I use it when I fork a session and want to know which branch actually got anywhere.

Simple keyword frequency, not semantic. Good enough.

**ATTACH:** `assets/mockups/diff.png`

---

## Tweet 10 (Claude Code skill)

There's also a /claude-picker skill for Claude Code itself.

Inside any Claude conversation: type /claude-picker, pick a session, Claude swaps to that context.

Useful when you wander away and want to jump back without leaving your terminal state.

**ATTACH (optional):** `assets/ai-generated/twitter/skill-card.png` — generate from image-prompts.md prompt #23

---

## Tweet 11 (The real insight)

Honestly, the tool wasn't the real win.

The real win: I started naming every session.

```
claude --name "auth-refactor"
claude --name "fix-race-condition"
claude --name "drizzle-migration"
```

2 seconds of effort. Now I find any conversation in under 3 seconds.

---

## Tweet 12 (CTA)

It's open source. ~800 lines of bash + python. Works in any terminal.

```
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker && bash ~/.claude-picker/install.sh
```

Needs: fzf 0.58+, python3 with rich (auto-installed).

Warp users get a + menu entry. Everyone gets Ctrl+P.

github.com/anshul-garg27/claude-picker

**ATTACH (optional):** `assets/gifs/hero.gif` again as the closing visual, OR let the link unfurl into the GitHub social preview (`assets/ai-generated/github/social-preview.png` if already set on the repo).

---

## Posting Notes
- Schedule tweet 1 at peak time (Tue-Thu 9-11 AM EST)
- Reply-chain the rest immediately after
- Quote-tweet #1 with the GIF again 6-8 hours later for second wave
- Pin the thread to your profile for a week
- Hashtags (only on tweet 1 or 12): #ClaudeCode #DeveloperTools #OpenSource
- If a tweet gets ratio'd or a reply has traction, turn the reply into its own thread
- Tweet 5 (--search) and tweet 7 (--tree) are the strongest standalone shares if you want to repost individual cards later

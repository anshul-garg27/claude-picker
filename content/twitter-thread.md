# Twitter/X Thread

Post Tuesday-Thursday, 9-11 AM EST for maximum reach.
ATTACH A DEMO GIF TO TWEET 1. Without it, engagement drops 3-5x.

---

## Tweet 1 (Hook — MUST stand alone)

You know that thing where you have 15 Claude Code sessions and can't remember which one had the fix you need?

I got tired of clicking through UUIDs. So I built a session browser.

432 lines. fzf. Works in any terminal.

[ATTACH: 15-second demo GIF — full flow from project picker to session resume]

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

---

## Tweet 3 (Solution — show, don't tell)

claude-picker gives you this instead:

- Pick a project (with session counts)
- Browse sessions (named ones on top)
- Preview the conversation before opening
- Fuzzy search to filter
- Ctrl+D to delete

Two-step flow. Under 500ms.

[ATTACH: screenshot of session list with preview panel]

---

## Tweet 4 (Interesting technical bit)

The fun part: reverse-engineering Claude Code's session storage.

Sessions are JSONL files in ~/.claude/projects/

But the path encoding is lossy — both / and _ become -

So /my_project and /my/project encode to the same thing.

Had to write 3 fallback strategies to crack it.

---

## Tweet 5 (The real insight)

Honestly, the tool wasn't the real win.

The real win: I started naming every session.

```
claude --name "auth-refactor"
claude --name "fix-race-condition"
```

2 seconds of effort. Now I find any conversation in under 3 seconds.

---

## Tweet 6 (CTA)

It's open source. Works in any terminal.

```
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker && bash ~/.claude-picker/install.sh
```

All you need: fzf + python3.

Warp users get a bonus tab config in the + menu.

github.com/anshul-garg27/claude-picker

---

## Posting Notes
- Schedule tweet 1 at peak time (Tue-Thu 9-11 AM EST)
- Reply-chain the rest immediately after
- Quote-tweet #1 with the GIF again 6-8 hours later for second wave
- Pin the thread to your profile for a week
- Hashtags (only on tweet 1 or 6): #ClaudeCode #DeveloperTools #OpenSource

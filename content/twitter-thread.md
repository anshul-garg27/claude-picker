# Twitter/X Thread

---

## Tweet 1 (Hook)

I use Claude Code every day.

But I had 50+ sessions scattered across projects with no way to find them.

So I built claude-picker — a session manager that lets you browse, preview, and resume any Claude Code conversation.

Open source. 432 lines. Zero config.

Here's how it works:

[ATTACH: demo GIF showing full flow]

---

## Tweet 2 (The Problem)

The problem with Claude Code sessions:

- `claude --resume` shows ALL sessions globally
- No preview — just UUIDs and timestamps
- No project filtering
- Can't delete old sessions

You end up clicking through 5 wrong sessions to find the one you need.

---

## Tweet 3 (The Solution)

claude-picker fixes this with a two-step flow:

Step 1: Pick your project (with activity bars)
Step 2: Browse sessions with names + message counts

Named sessions appear on top.
Unnamed sessions are grouped below.

All fuzzy-searchable with fzf.

[ATTACH: screenshot of session picker]

---

## Tweet 4 (Preview)

The killer feature: conversation preview.

Hover over any session and see the last few messages — before opening it.

No more guessing which session had that brilliant fix.

[ATTACH: screenshot showing preview panel]

---

## Tweet 5 (How It Works)

How it works under the hood:

I reverse-engineered Claude Code's session storage:

~/.claude/projects/ → JSONL files per session
~/.claude/sessions/ → metadata (name, cwd, pid)

Path encoding: / and _ both become -
Session names: stored as "custom-title" in JSONL
Filtering: "entrypoint" field distinguishes Claude CLI from SDK tools

---

## Tweet 6 (Install)

Install in 10 seconds:

```
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker && bash ~/.claude-picker/install.sh
```

Requirements: fzf + python3 (that's it)

Works in ANY terminal. Warp users get a bonus tab config.

---

## Tweet 7 (CTA)

Pro tip that changed my workflow:

Always name your Claude Code sessions:

```
claude --name "auth-refactor"
claude --name "fix-bug-123"
```

Takes 2 seconds. Saves you 10 minutes of searching later.

Star on GitHub if useful: github.com/anshul-garg27/claude-picker

---

## Suggested hashtags
#ClaudeCode #DeveloperTools #CLI #OpenSource #Anthropic #Productivity

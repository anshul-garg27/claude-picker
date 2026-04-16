# I Reverse-Engineered How Claude Code Stores Sessions and Built a 432-Line Tool to Browse Them

*How one frustrating afternoon led me to dig through JSONL files, crack a lossy path encoding scheme, and ship a session manager I now use 20 times a day.*

---

Let me be upfront: I didn't set out to build this.

Last Tuesday, I was three projects deep into Claude Code — debugging a Drizzle ORM migration in one, setting up MCP servers in another, and somewhere in between I'd had this brilliant 2am conversation about restructuring my entire auth middleware. 

I needed to get back to that auth conversation. So I ran `claude --resume`.

And got this:

```
? Pick a conversation to resume
  4a2e8f1c-9b3d-4e7a... (2 hours ago)
  b7c9d2e0-1f4a-8b6c... (3 hours ago)
  e5f8a3b1-7c2d-9e0f... (yesterday)
  ...14 more
```

UUIDs. Timestamps. Nothing else. No project names, no conversation preview, no way to tell which session had what. I clicked through four wrong ones before finding the auth conversation. Then I did something I do too often — I got nerd-sniped by the problem instead of actually doing my work.

Two hours later, I had a working session manager. (The auth middleware is still half-done. Don't judge.)

---

## What I Actually Built

**claude-picker** is a terminal tool that does three things:

1. Shows you every project directory where you've used Claude Code
2. Lets you browse sessions per project — with names, message counts, and timestamps
3. Shows a conversation preview before you open anything

It's 432 lines across three files. Bash, Python, and fzf. No frameworks, no build step, no config files. You run `claude-picker` and it works.

<!-- TODO: Insert terminal recording GIF here -->

---

## Digging Into Claude's Session Storage

The fun part was figuring out where Claude Code actually keeps your conversations. There's no documentation for this (or at least I couldn't find any). Here's what I found by poking around `~/.claude/`:

```
~/.claude/
├── projects/                    # Your sessions live here
│   ├── -Users-you-my-project/   # One directory per project
│   │   ├── abc123.jsonl         # One file per session
│   │   └── def456.jsonl
│   └── -Users-you-other-thing/
└── sessions/                    # Metadata (when it exists)
    └── 12345.json               # Name, cwd, start time
```

Three things tripped me up.

**The path encoding is lossy.** Claude turns `/Users/you/my_project` into `-Users-you-my-project`. Both `/` and `_` become `-`. Which means you can't reliably reverse it — `-my-project` could be `/my/project` or `/my_project` or `/my-project`. I ended up writing three fallback strategies to resolve paths: metadata lookup, scanning JSONL files for `cwd` fields, and encode-then-compare against known paths. Ugly? Yes. Works for 100% of my sessions? Also yes.

**Session names are buried.** When you do `claude --name "auth-refactor"`, the name isn't in any obvious metadata file. It's a `custom-title` entry inside the JSONL:

```json
{"type": "custom-title", "customTitle": "auth-refactor", "sessionId": "abc123..."}
```

I only found this by grep-ing through session files after nothing else worked.

**The entrypoint field saved me.** I also use Wibey (another tool built on Claude's SDK), and its sessions were showing up mixed in with my Claude Code conversations. Turns out every message has an `entrypoint` field — Claude Code uses `"cli"`, print mode uses `"sdk-cli"`, and SDK tools use `"sdk-ts"`. One filter and the noise was gone.

---

## How the Pieces Fit Together

The tool has three parts:

**The main script** (`claude-picker`) runs two sequential fzf pickers. First you pick a project, then you pick a session. It handles the directory discovery, path resolution, and launches Claude when you make a selection.

**The list builder** (`lib/session-list.sh`) reads JSONL files, extracts names and message counts, filters non-Claude sessions, and outputs ANSI-colored lines for fzf. Named sessions (the ones you created with `--name`) go on top under a "saved" header. Everything else goes under "recent."

**The preview renderer** (`lib/session-preview.py`) is where most of the annoying work went. Claude's JSONL files are noisy — hook outputs, system reminders, command metadata, tool call results. The preview strips all of that and shows you just the last few human messages. The conversation you actually care about.

I'm not going to pretend the code is beautiful. The path resolution function has three nested fallback loops. The JSONL noise filter is basically a list of string prefixes I don't want to see. But it's fast (under 500ms to scan 50+ sessions) and I haven't hit a bug since the first day.

---

## The Workflow Change Nobody Talks About

Here's the thing that actually surprised me. The tool itself isn't the insight — it's what happened to my workflow after I started using it.

I started naming sessions. Every time.

```bash
claude --name "drizzle-migration"
claude --name "fix-race-condition"  
claude --name "mcp-postgres-setup"
```

It takes two seconds. And now when I open claude-picker, I see this:

```
●  ui-redesign                    5m ago   2 msgs
●  auth-refactor                  2h ago  45 msgs
●  drizzle-migration              1d ago  28 msgs
○  session                        3h ago  12 msgs
○  session                        1d ago   6 msgs
```

The named sessions are instantly scannable. I can find any conversation in under three seconds. Before this, I was spending actual minutes clicking through UUIDs. Multiple times a day. The math works out to something embarrassing.

---

## Try It

If you use Claude Code and you've ever lost track of a session:

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker
bash ~/.claude-picker/install.sh
```

You need `fzf` and `python3` (that's it). Works in any terminal. If you use Warp, the installer adds a tab config automatically — you'll see "Claude Picker" in the `+` menu.

The whole thing is MIT licensed. 432 lines. If something breaks, you can read the entire codebase in ten minutes.

One request: start naming your Claude Code sessions. `claude --name "whatever"`. Future you will be grateful.

---

**GitHub:** [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

---

*Tags: Claude Code, Developer Tools, CLI, Terminal, Productivity, AI Coding*

# I Reverse-Engineered How Claude Code Stores Sessions and Built a Terminal Session Manager I Use Daily

*How one frustrating afternoon turned into an 800-line tool with stats, fork detection, full-text search, and a side habit of cracking lossy path encodings.*

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

What I didn't expect: the tool would keep growing. That first version was 432 lines. The version I use today is around 800 lines across nine Python scripts and a bash orchestrator. It now has stats, a tree view, fork detection, full-text search across every message I've ever sent, a diff view to compare two sessions, bookmarks, token and cost estimation, and a Claude Code skill so I can invoke it as `/claude-picker` from inside Claude itself.

None of that was planned. Each feature got added because I hit a wall, got annoyed, and spent an hour fixing it. This post is about those walls.

---

## What It Actually Does

**claude-picker** is a terminal tool for Claude Code sessions. The default flow is still simple:

1. Run `claude-picker`
2. Pick a project (all directories with Claude sessions, sorted by recent activity, with git branch and activity bars)
3. Pick a session (named ones on top, conversation preview in a side panel)
4. Claude resumes that session in your terminal

It's bash + python3 + fzf. No framework, no config file, no build step. The whole thing runs in under 500ms to list 50+ sessions.

Then there are the flags. These are what turned it from a picker into something I actually use every day for more than just resuming sessions.

---

## Digging Into Claude's Session Storage

The fun part was figuring out where Claude Code keeps your conversations. There's no documentation for this (or at least I couldn't find any). Here's what I found by poking around `~/.claude/`:

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

The tool has grown into nine Python scripts plus a bash entry point:

- `claude-picker` — the bash orchestrator (runs fzf pickers, dispatches to subcommands)
- `lib/session-list.py` — reads JSONL files, extracts names, message counts, timestamps, entrypoint filtering
- `lib/session-list.sh` — bash wrapper that pipes session-list.py output into fzf with the right ANSI colors
- `lib/session-preview.py` — the conversation preview renderer (Rich formatted, strips system noise)
- `lib/session-search.py` — full-text search across every message in every session
- `lib/session-export.py` — dumps a session to clean markdown
- `lib/session-stats.py` — the terminal dashboard with bar charts and cost estimation
- `lib/session-tree.py` — groups sessions by project, builds fork trees
- `lib/session-diff.py` — compares two sessions side by side
- `lib/session-bookmarks.py` — reads and writes the bookmark file

Each script is small and does one thing. The bash script decides which one to call based on flags. The theme (Catppuccin Mocha, 24-bit color) is shared across all of them so nothing looks out of place.

The fzf integration uses a few tricks worth mentioning. Section headers ("── saved ──" and "── recent ──") are just disabled rows that fzf skips when you press Enter. Delete-and-refresh is `execute-silent(...)+reload(...)`, which runs the delete in a subshell and reloads the list in place. And the new labeled borders in fzf 0.58+ let me render section titles directly on the border of the preview panel, which saves vertical space and looks clean.

I'm not going to pretend the code is beautiful. The path resolution function has three nested fallback loops. The JSONL noise filter is a list of string prefixes I don't want to see. But it's fast and I haven't hit a bug in weeks.

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
■  auth-refactor                  5m ago   2 msgs
●  drizzle-migration              2h ago  28 msgs
●  fix-race-condition             1d ago  45 msgs
○  session                        3h ago  12 msgs
○  session                        1d ago   6 msgs
```

The named sessions are instantly scannable. I can find any conversation in under three seconds. Before this, I was spending actual minutes clicking through UUIDs. Multiple times a day. The math works out to something embarrassing.

That blue `■` at the top? That's a bookmark — Ctrl+B on any session pins it above the saved list. I use it for the one or two conversations I'm actively working on. When I finish a feature, I unbookmark it and it drops back into the saved pool.

---

## The Flags That Changed How I Use Claude

After using the basic picker for a week, I started wanting more. The thing is, when you have 60+ Claude sessions across 8 projects, the picker alone is not enough. You want to search inside them. You want to know how much you've spent. You want to see how they relate to each other. Here's what got built.

### `--search`: Full-Text Across Everything

`claude-picker --search` searches every message in every session across every project. Type `drizzle migration`, get back a list of sessions ranked by match count, with a snippet of the matching message in the preview panel.

I built this the weekend I couldn't remember which project I'd debugged a particular race condition in. I knew I'd typed the phrase "the lock isn't released" at some point. `grep -r` through `~/.claude/projects/` worked but gave me one match per JSONL line, which meant hundreds of hits I had to wade through. The proper version groups hits by session, shows match count, and previews the matching lines with the query highlighted.

The one bug that took me embarrassingly long to fix: when you pick a session in `--search`, it needs to `cd` into the right project directory before launching Claude. Originally it launched Claude in the current shell's cwd, which meant opening a session about your auth middleware would resume it — but with its working directory pointing at whatever terminal you ran the search from. Claude would be confused about which files to look at. The fix: resolve the project dir from the JSONL's `cwd` field before exec-ing Claude.

### `--stats`: How Much Am I Actually Using This Thing

`claude-picker --stats` prints a terminal dashboard. It looks roughly like this:

```
 Total sessions        64
 Total tokens          ~1.4M  (rough estimate, content-length / 4)
 Estimated cost        ~$21   (blended rate, very rough)

 By project:
   architex              ████████████████████  23 sessions
   claude-picker         ████████████         14 sessions
   monorepo-infra        ████████              9 sessions
   ...

 Activity:
   today                 4
   this week             19
   older                 41

 Top 5 by tokens:
   1. auth-refactor (architex)          ~120k tokens   ~$1.80
   2. drizzle-migration (architex)       ~88k tokens   ~$1.32
   3. k8s-deployment (monorepo-infra)    ~71k tokens   ~$1.06
   ...
```

A few things about how this works. The token count is a rough approximation — I sum the content length of every message in every JSONL and divide by 4. That's the cheap blended estimate you see in OpenAI docs. It's not exact, but it's in the right order of magnitude, and across 60+ sessions the errors wash out.

The cost number is even rougher. I multiply by 0.000015 — a conservative blended rate that's somewhere between input and output token prices for Sonnet. Only sessions over 10k tokens get a cost line in the individual preview (smaller sessions just say "negligible") because sub-cent numbers aren't useful. The top-5 list helps me spot sessions where I probably should have started fresh instead of letting the context grow to 100k+ tokens.

I don't use this for budgeting. I use it for the "oh so that's where my time went this month" feeling.

### `--tree`: Sessions Grouped by Project, Forks Included

Claude Code has a `/branch` command (and a `--fork-session` flag on the CLI) that forks a session. When you fork, the new session has a `forkedFrom` field pointing at the parent's session ID. Until I added `--tree`, I had no way to see that relationship from outside Claude.

```
architex  (git: main, disk: 12 MB)
├── auth-refactor                          2h ago   45 msgs
│   └── auth-refactor-jwt-variant          1h ago   12 msgs   (forked)
├── drizzle-migration                      1d ago   28 msgs
│   ├── drizzle-migration-v2               8h ago   18 msgs   (forked)
│   └── drizzle-migration-postgres-only    4h ago    9 msgs   (forked)
└── session                                3h ago   12 msgs

monorepo-infra  (git: k8s-cleanup)
├── k8s-deployment                         2d ago   71 msgs
└── k8s-deployment-helm-branch             1d ago   34 msgs   (forked)
```

The tree builder reads every session's `forkedFrom` field and walks the resulting graph. If a parent session is missing (deleted), the child shows up as a root with an orphan marker. The tree uses Unicode box-drawing characters and colors roots with the Catppuccin palette, so it looks clean in any truecolor terminal.

This was the feature I didn't know I needed. I'd been forking sessions for weeks without thinking about it, and when I finally ran `--tree` I had three different branches off a single refactor conversation that I'd totally forgotten about. One of them had a working implementation the parent didn't.

### `--diff`: Compare Two Sessions

This one's simple. Pick two sessions, get a side-by-side view:

- Common topics (keywords that appear in both)
- Unique topics per session
- Conversation previews from each

I use this when I fork a session and want to know which branch actually made progress, or when I have two separate conversations that touch the same area and I want to consolidate. It's not doing any deep semantic comparison — just token frequency on message content, filtered against a stopword list. Good enough.

### `--pipe`: For Scripting

`claude-picker --pipe` outputs the selected session ID to stdout and exits. Useful when you want to compose with other commands:

```bash
claude --resume $(claude-picker --pipe)
```

I didn't think I'd use this much. Then I wrote a shell alias that runs tests against the working directory of the session I pick:

```bash
cs-test() {
  local dir=$(claude-picker --pipe --return-cwd)
  (cd "$dir" && npm test)
}
```

Now I can run tests for any project I have a Claude session in, without knowing where it lives on disk.

---

## Keyboard Shortcuts Inside the Picker

Once the picker is open, a few keys do heavy lifting:

- `Ctrl+B` — toggle bookmark. Bookmarked sessions get a blue `■` and pin to the very top.
- `Ctrl+E` — export the selected session to markdown in `~/Desktop/claude-exports/`. Clean format, no system noise, ready to share.
- `Ctrl+D` — delete the selected session. `execute-silent(rm)+reload(list)` keeps you in the picker.
- `Ctrl+P` — global shortcut installed by the installer. Launches the picker from any prompt.
- Just typing — fuzzy filter. Standard fzf behavior.

`Ctrl+P` is the one I set up last and miss the most when I'm on a machine without it. The installer detects zsh vs bash and drops the keybinding into the right rc file. From any prompt, in any project, `Ctrl+P` opens the full picker.

---

## The Claude Code Skill

Claude Code supports user-installed skills that show up as slash commands. I wrote one called `/claude-picker`. Inside any Claude session, I can type `/claude-picker` and it runs the tool, lets me pick a session, and swaps the current conversation for the selected one. Useful when I've wandered away from what I was doing and want to jump back to another context without leaving my terminal state.

The skill is about 40 lines of instructions plus a script invocation. It's in the repo under `skills/claude-picker/`.

---

## Small Things That Add Up

A bunch of little quality-of-life features ended up mattering more than I expected:

- **Git branch next to project name.** Every project in the picker shows its current git branch. Turns out when you have two checkouts of the same repo for different features, this is the fastest way to tell them apart.
- **Activity bars.** `████` next to project names, scaled to the most active project. At a glance I know which project has the most recent work.
- **Age warnings.** Sessions get a peach-colored timestamp after 7 days and a red warning icon after 30. Encourages me to actually close old conversations instead of letting them pile up.
- **Cost annotation** on sessions over 10k tokens, so the expensive ones are obvious.
- **Disk usage + session count** in the picker header. One-line sanity check.
- **Auto-naming for unnamed sessions.** If you didn't name it, the label is the first user message (truncated). Way better than "session".
- **Relative time.** "5m ago" instead of absolute timestamps. Timestamps are fine for logs, relative time is better for human scanning.
- **Section headers.** "── saved ──" and "── recent ──" separate the two lists without adding noise.

None of these is a killer feature. The aggregate is what makes the tool pleasant to use.

---

## Things That Went Wrong

Worth mentioning what didn't work so people don't hit the same things:

**The lossy path encoding still bites.** My three-fallback resolver covers every session I've seen, but I'd bet there's a pathological case where it's wrong and I haven't noticed. If you ever see a session open in the wrong directory, that's this bug.

**fzf 0.58+ is a hard dependency now.** The labeled borders are genuinely nicer than the alternative, so I didn't want to write a fallback path. Older fzf will error out with a clear message from the installer.

**Cost estimation is vibes, not accounting.** Don't paste the numbers into an expense report. They're within an order of magnitude, good enough for "am I using this too much" but not for anything precise.

**The search index is rebuilt on every query.** For 64 sessions it takes ~200ms, which is fine. For 1000+ sessions it would be slow. I'll add a cache if anyone actually hits that.

---

## Try It

If you use Claude Code and you've ever lost track of a session:

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker
bash ~/.claude-picker/install.sh
```

You need `fzf` 0.58+ and `python3` with the `rich` package (the installer handles `rich` for you). Works in any terminal. If you use Warp, the installer adds a tab config automatically — you'll see "Claude Picker" in the `+` menu. The installer also adds `Ctrl+P` to your shell so you can invoke the picker from anywhere.

The whole thing is MIT licensed. Around 800 lines of Python and bash. If something breaks, you can read the entire codebase in an afternoon.

One request: start naming your Claude Code sessions. `claude --name "whatever"`. Future you will be grateful.

---

**GitHub:** [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

---

*Tags: Claude Code, Developer Tools, CLI, Terminal, Productivity, AI Coding, fzf, Python, Bash*

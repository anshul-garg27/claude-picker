# Community Posts

---

## Reddit: r/ClaudeAI

**Title:** I kept losing track of my Claude Code sessions, so I dug into how they're stored and built a browser for them (432 lines, open source)

**Body:**

I've been using Claude Code daily for the past few months across 4-5 projects. My biggest frustration wasn't the model or the tooling — it was finding old sessions.

`claude --resume` shows a flat list of UUIDs with timestamps. That's it. No project filtering, no preview, no names. Every time I needed to get back to a specific conversation, I'd click through 3-4 wrong sessions before finding the right one. Sometimes I'd give up and start a new one, losing all that context.

So last week I got nerd-sniped and spent an afternoon reverse-engineering how Claude Code stores sessions.

Turns out they're JSONL files in `~/.claude/projects/`, organized by an encoded version of your directory path. Session names (if you use `claude --name`) are buried as `custom-title` entries inside the JSONL. And there's an `entrypoint` field that lets you filter out SDK-based tools.

I built **claude-picker** — a terminal tool that:

- Shows all your project directories that have Claude sessions (sorted by recent activity)
- Lists sessions per project with names, message counts, and relative timestamps
- Previews the last few messages in a side panel so you know what a session is about before opening it
- Fuzzy search with fzf
- Ctrl+D to delete sessions you don't need
- Filters out non-Claude sessions automatically (I use Wibey too, those were cluttering things up)

432 lines. Bash + Python + fzf. Works in any terminal.

Install:

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker
bash ~/.claude-picker/install.sh
```

The thing that actually changed my workflow more than the tool itself: I started naming every session. `claude --name "auth-refactor"` takes two seconds and makes sessions instantly findable.

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

Still rough around the edges but it works. Would love to hear if others have the same pain point or if there's a better way I'm missing.

---

## Reddit: r/commandline

**Title:** Built a session browser for Claude Code with fzf — two-step project→session picker with conversation previews

**Body:**

Quick share for anyone using Claude Code from the terminal.

Claude Code saves sessions as JSONL files in `~/.claude/projects/` but the built-in `--resume` only shows UUIDs. No filtering, no preview.

I put together a tool that:

1. Discovers all directories with Claude sessions, resolves their real paths (Claude's encoding is lossy — `/` and `_` both become `-`)
2. Presents a fzf picker with relative timestamps and session counts
3. After picking a project, shows sessions with names, message counts, and a conversation preview in fzf's `--preview` window
4. `ctrl-d` to delete, fuzzy search to filter, `execute-silent` + `reload` for the delete-refresh combo

The technically interesting bits:

- Three-layer path resolution (metadata lookup → JSONL `cwd` field scan → encode-and-compare)
- ANSI 256-color output with visual hierarchy (named sessions green + bold, unnamed dimmed)
- Preview renderer that strips system noise from JSONL (hook outputs, command metadata, system reminders)
- Entrypoint-based filtering to separate Claude CLI sessions from SDK tool sessions

432 lines across 3 files. Bash orchestrator + bash list builder + Python preview renderer.

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker
bash ~/.claude-picker/install.sh
```

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

---

## Reddit: r/terminal

**Title:** Terminal UI for browsing Claude Code sessions — fzf with 256-color output, conversation preview panel, and activity bars

**Body:**

Sharing a tool I built for managing Claude Code sessions. Posting here because the terminal UI aspect might be interesting regardless of whether you use Claude Code.

The design uses:

- fzf with `--preview` pointing to a Python renderer
- 256-color ANSI codes (`\033[38;5;XXXm`) for a refined palette — soft cyan for project names, warm yellow for saved indicators, dimmed gray for timestamps
- `--bind` with `execute-silent` + `reload` for delete-and-refresh in one keystroke
- `--delimiter` and `--with-nth` to hide internal IDs while keeping them accessible for the preview command
- Activity bars (`████████`) rendered inline showing session counts per project
- Section headers ("saved" / "recent") as non-functional separator rows
- `--color` overrides for pointer, prompt, border, and gutter to achieve a cohesive dark-theme look

The preview panel extracts conversation messages from JSONL files and renders them with role-colored labels (cyan for user, yellow for AI), stripping system noise.

432 lines. bash + python3 + fzf.

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

Happy to answer questions about the fzf configuration — took a fair amount of trial and error to get `--bind`, `--preview`, and ANSI colors to play nice together.

---

## Hacker News: Show HN

**Title:** Show HN: Claude-Picker – Browse and resume Claude Code sessions from your terminal

**URL:** https://github.com/anshul-garg27/claude-picker

**Text:**

Hi HN. I've been using Claude Code daily and kept running into the same problem: the built-in --resume shows a flat list of session UUIDs with no project filtering or preview.

I spent an afternoon reverse-engineering how Claude Code stores sessions. They're JSONL files in ~/.claude/projects/, keyed by an encoded directory path. Session names are custom-title entries in the JSONL. Each message has an entrypoint field that distinguishes CLI sessions from SDK-based tools.

claude-picker is a 432-line tool (bash + python3 + fzf) that reads these files and gives you:

- A project picker showing all directories with Claude sessions
- A session browser with names, message counts, and relative timestamps
- A conversation preview panel (strips system noise from JSONL, shows the last few user/AI messages)
- Fuzzy search and Ctrl+D to delete

The technically tricky part was path resolution. Claude's encoding replaces both / and _ with -, making it lossy. I use three fallback strategies: metadata lookup, scanning JSONL for cwd fields, and encode-then-compare against known paths.

No dependencies beyond fzf and python3. Works in any terminal. MIT licensed.

I'd appreciate feedback, especially from anyone who's worked with Claude Code's session format. Curious if there's a cleaner way to resolve the encoded paths.

---

## Warp Community / Discord

**Title:** Claude Picker — Browse Claude Code sessions from the + menu

**Body:**

Hey Warp fam,

Built a session manager for Claude Code that plugs right into Warp's tab configs.

**The problem:** Claude Code's `--resume` shows UUIDs across all projects. When you have 15+ sessions across multiple repos, finding the right one is painful.

**What it does:** Click `+` → "Claude Picker" and you get:

1. A project picker (all directories where you've used Claude Code)
2. A session browser with conversation preview, fuzzy search, and delete

Named sessions (created with `claude --name "something"`) show on top. Everything is fzf-powered with 256-color output.

**Install:**

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker
bash ~/.claude-picker/install.sh
```

The installer auto-detects Warp and drops a tab config into `~/.warp/tab_configs/`. You'll see "Claude Picker" in the `+` menu immediately.

Also works in any other terminal — the Warp integration is a bonus.

GitHub: [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

---

## Posting Strategy

**Order:**
1. Reddit r/ClaudeAI first (Tuesday morning) — biggest relevant audience
2. Twitter thread same day, 2-3 hours later
3. Hacker News Show HN next morning (Wednesday 8-11 AM ET)
4. Reddit r/commandline and r/terminal same day as HN
5. Warp community anytime after

**Critical:**
- Attach demo GIF to every Reddit post and tweet 1
- On Reddit, engage with EVERY comment in the first 2 hours
- On HN, answer technical questions with depth
- Don't cross-post the exact same text — each post is written for its platform
- Don't post to all subreddits on the same day (looks spammy)

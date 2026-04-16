# I Built a Session Manager for Claude Code Because I Was Drowning in 50+ Conversations

## I kept losing my best Claude Code sessions. So I reverse-engineered how Claude stores them and built a tool to fix it.

---

If you use Claude Code daily, you know the feeling.

You're deep in a debugging session. Claude just helped you untangle a gnarly race condition. The fix is working. You close the terminal, grab lunch, come back — and now you have no idea which session that was.

`claude --resume` shows you a list of UUIDs. Helpful.

```
? Pick a conversation to resume
  4a2e8f1c-9b3d-4e7a... (2 hours ago)
  b7c9d2e0-1f4a-8b6c... (3 hours ago)  
  e5f8a3b1-7c2d-9e0f... (yesterday)
```

Which one had the race condition fix? Which one was the auth refactor? Which one was that brilliant idea at 2am that you definitely need to find again?

You click through three wrong sessions before finding it. Or worse — you give up and start a new one, losing all that context.

**I got tired of this.** So I built `claude-picker`.

---

## What is claude-picker?

It's a terminal-native session manager for Claude Code. In 432 lines of bash and Python, it gives you:

1. **A project picker** — see every directory where you've used Claude Code
2. **A session browser** — with names, timestamps, message counts
3. **A conversation preview** — see the last few messages *before* opening
4. **Fuzzy search** — powered by fzf, type to filter instantly
5. **Delete sessions** — Ctrl+D to clean up what you don't need

The whole thing runs in your terminal. No Electron app, no web UI, no dependencies beyond `fzf` and `python3`.

---

## The Problem in Detail

Claude Code is incredible for development work. But it has a session management blind spot.

**Problem 1: No project filtering.** `claude --resume` shows sessions from every directory. If you work on multiple projects (who doesn't?), you're scrolling through a mixed bag of unrelated conversations.

**Problem 2: No preview.** You can't see what a session was about until you open it. Each session shows a timestamp and a UUID. That's it.

**Problem 3: No names by default.** Claude Code supports `--name` flags, but most people don't use them. So all sessions look identical.

**Problem 4: Mixed sessions.** If you use other tools built on Claude's SDK (like Wibey or custom agents), those sessions appear in the same list. There's no way to filter.

---

## Reverse-Engineering Claude's Session Storage

The first thing I needed to figure out: where does Claude Code actually store sessions?

After some digging, I found the answer:

```
~/.claude/
├── projects/              # Session data, organized by directory
│   ├── -Users-you-project-a/
│   │   ├── abc123.jsonl   # Each session is a JSONL file
│   │   └── def456.jsonl
│   └── -Users-you-project-b/
│       └── ghi789.jsonl
└── sessions/              # Session metadata (name, cwd, pid)
    ├── 12345.json
    └── 67890.json
```

**Key discoveries:**

**1. Path encoding.** Claude encodes directory paths by replacing `/` and `_` with `-`. So `/Users/you/my_project` becomes `-Users-you-my-project`. This is important for mapping sessions back to real directories.

**2. JSONL format.** Each session is a JSONL (JSON Lines) file where every line is a message or event. User messages, assistant responses, tool calls — everything is here.

**3. Session names.** When you use `claude --name "something"`, the name is stored as a `custom-title` entry inside the JSONL file:

```json
{"type": "custom-title", "customTitle": "auth-refactor", "sessionId": "abc123..."}
```

**4. Metadata files.** The `~/.claude/sessions/` directory has JSON files with metadata: the real `cwd`, `startedAt` timestamp, and `name` (if set from the CLI). But these only exist for sessions that were active in the current boot — older sessions don't have metadata.

**5. Entrypoint field.** Each message has an `entrypoint` field. Claude Code CLI uses `"cli"` or `"sdk-cli"`. SDK-based tools use `"sdk-ts"`. This is how I filter out non-Claude sessions.

---

## The Architecture

The tool is split into three scripts:

### 1. `claude-picker` (main entry point)

The orchestrator. It runs two sequential fzf pickers:

- **Step 1:** Discover all project directories with Claude sessions, resolve their real paths, and present them in a fuzzy-searchable list.
- **Step 2:** For the selected project, list all sessions with names, timestamps, and message counts.

### 2. `lib/session-list.sh`

Builds the formatted session list for fzf. It:
- Reads all `.jsonl` files in the project directory
- Extracts session names from `custom-title` entries
- Falls back to metadata files for names
- Counts user/assistant messages
- Filters out non-Claude sessions by checking `entrypoint`
- Separates named ("saved") and unnamed ("recent") sessions
- Applies ANSI colors for visual hierarchy

### 3. `lib/session-preview.py`

Generates the conversation preview panel. When you hover over a session in fzf, it:
- Reads the JSONL file
- Extracts the last 8 meaningful user/AI messages
- Filters out system noise (hook outputs, command metadata)
- Formats with colors: cyan for "you", yellow for "ai"

---

## The Hardest Parts

### Resolving real directory paths

Claude's path encoding is lossy. Both `/` and `_` become `-`, so you can't reliably reverse the encoding. `-Users-you-my-project` could be `/Users/you/my-project` or `/Users/you/my_project`.

My solution uses three fallback strategies:

1. **Match session IDs against metadata** — session metadata files have the real `cwd`. Find any session ID in the JSONL, look it up in metadata.
2. **Read `cwd` from JSONL entries** — some messages have a top-level `cwd` field with the real path.
3. **Encode-and-compare** — take all known `cwd` values from metadata, encode them, and see if any match the directory name.

This three-layer approach resolves paths for 100% of my sessions.

### Filtering noise from previews

Claude Code's JSONL files contain *everything* — hook outputs, system reminders, command metadata, tool results. The raw content looks like:

```
<local-command-caveat>Caveat: The messages below were generated...
<system-reminder>The task tools haven't been used recently...
<bash-stdout>npm install completed</bash-stdout>
```

The preview script has a noise filter that strips all of this, showing only the actual human conversation.

### fzf integration

fzf is incredibly powerful but tricky to configure for this use case:

- `--delimiter` and `--with-nth` to hide session IDs from the display while keeping them accessible
- `--preview` with a shell command that extracts the session ID and passes it to the preview script
- `--bind` for Ctrl+D delete with `execute-silent` + `reload` to delete and refresh in one action
- `--color` with 256-color codes for a polished visual design
- `--layout=reverse` to put the cursor on the most recent item

---

## How to Use It

### Install

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker && bash ~/.claude-picker/install.sh
```

### Run

```bash
claude-picker
```

### Pro tips

**Name your important sessions:**

```bash
claude --name "auth-refactor"
claude --name "fix-bug-123"
```

Named sessions appear at the top of the picker with a `●` indicator. This is the single best workflow improvement — you'll always find your important sessions instantly.

**Customize Claude flags:**

```bash
export CLAUDE_PICKER_FLAGS=""                    # no special flags
export CLAUDE_PICKER_FLAGS="--model sonnet"      # use a specific model
```

**Warp terminal users** get a bonus: the installer automatically adds a tab config so you can access claude-picker from the `+` menu.

---

## What I Learned

Building this taught me a few things:

1. **Claude Code's session format is well-structured.** JSONL is a great choice — you can stream-read without loading entire files, and each line is independently parseable.

2. **fzf is an underrated UI framework.** With `--preview`, `--bind`, and ANSI colors, you can build remarkably polished TUI experiences with just shell scripts.

3. **The best developer tools solve small, specific pain points.** This tool doesn't do anything revolutionary. It just makes session management — something you do dozens of times a day — feel effortless.

4. **Always name your sessions.** Seriously. `claude --name "descriptive-name"` takes two seconds and saves you minutes of searching later.

---

## Try It

The tool is open source and works in any terminal:

**GitHub:** [github.com/anshul-garg27/claude-picker](https://github.com/anshul-garg27/claude-picker)

```bash
git clone https://github.com/anshul-garg27/claude-picker.git ~/.claude-picker && bash ~/.claude-picker/install.sh
```

If you use Claude Code daily, give it a try. And if you find it useful, a star on GitHub would mean a lot.

---

*Built by [Anshul Garg](https://github.com/anshul-garg27). Licensed under MIT.*

*Tags: Claude Code, Anthropic, CLI, Developer Tools, fzf, Terminal, Productivity*

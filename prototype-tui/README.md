# claude-picker — Textual prototype

A **standalone experimental prototype** of the `claude-picker` session
picker built with [Textual](https://textual.textualize.io). The existing
fzf-based picker (at `lib/session-list.py` + `claude-picker` bash) is
untouched — this prototype lives beside it so you can compare.

The goal: see whether a Textual UI can match the polish of the HTML
mockup the website showed, without giving up the keyboard-first feel of
the current fzf picker.

## What's in here

| Path                              | Purpose                                                      |
|-----------------------------------|--------------------------------------------------------------|
| `../lib/session-picker-tui.py`    | The Textual app (main)                                       |
| `../lib/session-picker-tui.tcss`  | The Textual CSS stylesheet (Catppuccin Mocha)                |
| `../scripts/demo-mode-tui.sh`     | Launches the app with hard-coded demo data (for GIFs/tapes)  |
| `requirements.txt`                | Pinned `textual>=0.86.0`, `rich>=13.7.0`                     |
| `screenshots/`                    | SVG captures of the app in a few states                      |

## Why it exists

The current production `claude-picker` is a **bash + fzf + Rich**
stack: very fast, very lightweight, but constrained by fzf's rendering
model (left pane is fzf's list, right pane is the preview command's
stdout). That constraint makes some things awkward:

- No live reactivity between the preview and list (preview is a
  subprocess per selection).
- No ability to have UI chrome outside what fzf draws.
- Limited input handling (one text field, no modals, no toasts).

The **Textual** prototype shows what's possible if we lift that
constraint: a single process owns the whole screen, selection changes
are instant, the preview fades in, and we can layer toasts/modals on
top.

## Install

Textual requires **Python 3.10+**. macOS's system `python3` is 3.9, so
install a newer one:

```bash
# macOS (Homebrew)
brew install python@3.12

# Install the prototype's deps for that interpreter
python3.12 -m pip install --user -r prototype-tui/requirements.txt
```

Linux and Windows users can use their distro's `python3` if it's already
3.10+ (`python3 --version` to check).

## Run

Against your real sessions at `~/.claude/projects/`:

```bash
python3.12 lib/session-picker-tui.py
```

With hard-coded demo data (good for recording / sharing screenshots):

```bash
scripts/demo-mode-tui.sh
# or directly:
python3.12 lib/session-picker-tui.py --demo
```

## Keys

| Key               | Action                                            |
|-------------------|---------------------------------------------------|
| `↑ / ↓` or `j/k`  | Navigate the session list                         |
| `PgUp / PgDn`     | Page the list                                     |
| `Home / End`      | Top / bottom                                      |
| Type letters      | Live-filter the list (matches name + first user)  |
| `Esc`             | Clear the filter                                  |
| `Enter`           | "Resume" the selected session (exits + prints id) |
| `Ctrl+B`          | Bookmark (toast: coming in v2)                    |
| `Ctrl+E`          | Export (toast: coming in v2)                      |
| `Ctrl+D`          | Delete (toast: coming in v2)                      |
| `q` or `Ctrl+C`   | Quit                                              |

## How it differs from the production picker

| Dimension          | Production (`claude-picker`)             | This prototype                             |
|--------------------|------------------------------------------|--------------------------------------------|
| Stack              | bash + fzf + Rich                        | Python + Textual                           |
| Startup            | ~50 ms                                   | ~400–800 ms (Python + Textual import)      |
| Preview update     | subprocess per selection (~80 ms)        | in-process (instant)                       |
| Filter             | fzf fuzzy match                          | substring (token-AND) — simpler on purpose |
| Actions            | Bookmark/Export/Delete via key handlers  | Stubbed as toast (prototype only)          |
| Dependencies       | fzf, Python 3.9+, Rich                   | Python 3.10+, Textual                      |
| Works over SSH     | Yes, any terminal                        | Yes, any xterm-256color terminal           |
| "Can be styled"    | ANSI sequences in each script            | Single `.tcss` file (CSS-like)             |

## What the prototype deliberately does NOT do

This is a shape-check, not a feature-complete rewrite. The following
are explicitly cut:

- Bookmarks, export, delete (shown as toasts).
- Project picker screen (the two-step flow is collapsed into one —
  sessions are loaded across **all** projects and the project name
  appears under the session title in the preview pane).
- Session tree / fork view.
- Stats dashboard (`session-stats.py` remains the source of truth).
- Session deletion confirmation modal.

If we migrate, those land in v2 after the shape is agreed.

## The decision this prototype exists to inform

After trying it (and the production picker) side-by-side, the question
is:

1. **Stay with fzf** — Textual's startup cost, dependency footprint,
   and Python 3.10 requirement aren't worth the visual upgrade.
2. **Migrate to Textual** — The reactivity and polish justify the
   added complexity; rewrite the picker, drop fzf as a dependency.
3. **Hybrid** — Keep fzf as the default (fast, lean), add a
   `claude-picker --tui` flag that launches the Textual path for users
   who prefer it.

Option 3 is the most conservative — no one loses what they have.
Option 2 is the cleanest long-term, but requires committing to Python
3.10+ as a base.

Look at the screenshots under `screenshots/` for the visual delta, then
decide.

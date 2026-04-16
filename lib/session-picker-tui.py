#!/usr/bin/env python3
"""
claude-picker — Textual prototype.

A two-pane session picker, reimagined in Textual. Mirrors the website
mockup (list on the left, live preview on the right, filter bar on top,
key-hints footer).

This is a STANDALONE prototype — it does NOT replace the existing
lib/session-list.py / fzf-based picker. Run it directly:

    python3 lib/session-picker-tui.py

Data source: ~/.claude/projects/*/*.jsonl  (same as the real picker).
Pricing:      mirrored from lib/session-stats.py so cost estimates match.
"""

from __future__ import annotations

import glob
import json
import os
import re
import sys
import time
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any, Optional

# ── Soft-fail if Textual isn't installed ─────────────────────────────
try:
    from textual.app import App, ComposeResult
    from textual.binding import Binding
    from textual.containers import Horizontal, Vertical
    from textual.reactive import reactive
    from textual.widgets import (
        DataTable,
        Footer,
        Input,
        Label,
        Static,
    )
    from rich.text import Text
    from rich.console import RenderableType
except ImportError:
    print(
        "Textual is required for this prototype.\n"
        "Install it with:  python3 -m pip install --user 'textual>=0.86.0'\n"
        "(Textual needs Python 3.10+ — check with `python3 --version`.)",
        file=sys.stderr,
    )
    sys.exit(1)


# ── Pricing (copied verbatim from lib/session-stats.py) ──────────────

def _rates(inp: float, out: float, cw5: float, cw1: float, cr: float) -> dict:
    return dict(
        input=inp / 1_000_000,
        output=out / 1_000_000,
        cache_w5=cw5 / 1_000_000,
        cache_w1=cw1 / 1_000_000,
        cache_r=cr / 1_000_000,
    )


MODEL_PRICES = [
    ("claude-opus-4",     _rates( 5.00, 25.00,  6.25, 10.00, 0.50)),
    ("claude-3-opus",     _rates(15.00, 75.00, 18.75, 30.00, 1.50)),
    ("claude-sonnet-4",   _rates( 3.00, 15.00,  3.75,  6.00, 0.30)),
    ("claude-3-7-sonnet", _rates( 3.00, 15.00,  3.75,  6.00, 0.30)),
    ("claude-3-5-sonnet", _rates( 3.00, 15.00,  3.75,  6.00, 0.30)),
    ("claude-haiku-4",    _rates( 1.00,  5.00,  1.25,  2.00, 0.10)),
    ("claude-3-5-haiku",  _rates( 0.80,  4.00,  1.00,  1.60, 0.08)),
]
FALLBACK_RATES = MODEL_PRICES[0][1]


def rates_for(model: str) -> Optional[dict]:
    if not model or model == "<synthetic>":
        return None
    for prefix, rates in MODEL_PRICES:
        if model.startswith(prefix):
            return rates
    return FALLBACK_RATES


def short_model(model: str) -> str:
    """Compact model tag used in the list row."""
    if not model:
        return "—"
    m = model.lower()
    if "opus" in m:
        return "opus"
    if "sonnet" in m:
        return "sonnet"
    if "haiku" in m:
        return "haiku"
    return model.split("-")[1] if "-" in model else model


def model_color(tag: str) -> str:
    return {
        "opus":   "#FAB387",  # peach
        "sonnet": "#94E2D5",  # teal
        "haiku":  "#89B4FA",  # blue
    }.get(tag, "#A6ADC8")


# ── Data model ────────────────────────────────────────────────────────

@dataclass
class Session:
    session_id: str
    name: str                  # custom-title OR first user message excerpt
    has_custom_name: bool
    project: str
    project_dir: str           # encoded projects/ dir basename
    cost: float
    model: str                 # "opus" / "sonnet" / "haiku" / ...
    model_full: str            # full model id for preview stats
    msg_count: int
    total_tokens: int
    mtime: float
    file_path: str
    preview_text: str          # pre-rendered conversation preview
    first_user: str            # for filter matching
    search_blob: str = ""      # lowercased name + project + first_user


NOISE_PREFIXES = (
    "<local-command", "<command-name>", "<bash-",
    "<system-reminder>", "[Request inter", "<command-message>",
    "<user-prompt",
)


def is_noise(text: str) -> bool:
    t = text.lstrip()
    return any(t.startswith(p) for p in NOISE_PREFIXES)


def human_age(ts: float) -> str:
    diff = time.time() - ts
    if diff < 60:
        return "now"
    if diff < 3600:
        return f"{int(diff / 60)}m"
    if diff < 86400:
        return f"{int(diff / 3600)}h"
    if diff < 86400 * 2:
        return "yd"
    if diff < 86400 * 7:
        return f"{int(diff / 86400)}d"
    if diff < 86400 * 30:
        return f"{int(diff / 86400 / 7)}w"
    return datetime.fromtimestamp(ts).strftime("%b %d")


def format_cost(c: float) -> str:
    if c >= 1000:
        return f"${c:,.0f}"
    return f"${c:,.2f}"


def format_tokens(t: int) -> str:
    if t >= 1_000_000:
        return f"{t / 1_000_000:.1f}M"
    if t >= 1_000:
        return f"{t / 1_000:.1f}k"
    return str(t)


def decode_project(dir_name: str, projects_dir: str) -> str:
    """Best-effort decode of ~/.claude/projects/<encoded>/ back to a project name."""
    # session metadata often carries cwd; we don't need to resolve it — keep it simple.
    parts = dir_name.strip("-").split("-")
    return parts[-1] if parts else dir_name


def load_sessions(projects_dir: str, preview_limit: int = 12) -> list[Session]:
    """Walk ~/.claude/projects/*.jsonl, return sessions sorted by mtime desc."""
    sessions: list[Session] = []
    if not os.path.isdir(projects_dir):
        return sessions

    for project_enc in os.listdir(projects_dir):
        full = os.path.join(projects_dir, project_enc)
        if not os.path.isdir(full):
            continue
        jsonls = glob.glob(os.path.join(full, "*.jsonl"))
        if not jsonls:
            continue
        project_name = decode_project(project_enc, projects_dir)
        for jf in jsonls:
            try:
                sess = _parse_session(jf, project_name, project_enc, preview_limit)
                if sess:
                    sessions.append(sess)
            except Exception:
                # Skip malformed sessions silently — prototype; no blockers.
                continue

    sessions.sort(key=lambda s: s.mtime, reverse=True)
    return sessions


def _parse_session(
    jf: str,
    project_name: str,
    project_enc: str,
    preview_limit: int,
) -> Optional[Session]:
    file_size = os.path.getsize(jf)
    if file_size < 200:
        return None

    session_id = os.path.basename(jf).replace(".jsonl", "")
    mtime = os.path.getmtime(jf)

    custom_title: Optional[str] = None
    first_user: str = ""
    msg_count = 0
    total_tokens = 0
    cost = 0.0
    models_seen: dict[str, int] = {}   # model_full -> message count
    preview_items: list[tuple[str, str]] = []   # [(role, text), ...]

    try:
        with open(jf, "r", encoding="utf-8", errors="replace") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    data = json.loads(line)
                except Exception:
                    continue

                t = data.get("type")

                if t == "custom-title" and data.get("customTitle"):
                    custom_title = data["customTitle"]
                    continue

                if t not in ("user", "assistant"):
                    continue

                msg = data.get("message") or {}
                role = msg.get("role")
                if role not in ("user", "assistant"):
                    continue
                msg_count += 1

                # Extract text content
                content = msg.get("content", "")
                text = ""
                if isinstance(content, str):
                    text = content
                elif isinstance(content, list):
                    for item in content:
                        if isinstance(item, dict) and item.get("type") == "text":
                            text = item.get("text", "")
                            if text:
                                break
                text = text.strip()

                if role == "user":
                    if text and not is_noise(text):
                        if not first_user:
                            first_user = text[:200]
                        if len(preview_items) < preview_limit:
                            preview_items.append(("user", text[:280]))
                else:  # assistant
                    if text and len(preview_items) < preview_limit:
                        preview_items.append(("claude", text[:280]))

                    # Cost + tokens
                    model_full = msg.get("model") or ""
                    r = rates_for(model_full)
                    if r is None:
                        continue
                    models_seen[model_full] = models_seen.get(model_full, 0) + 1
                    usage = msg.get("usage") or {}
                    it = int(usage.get("input_tokens", 0))
                    ot = int(usage.get("output_tokens", 0))
                    cr = int(usage.get("cache_read_input_tokens", 0))
                    cc = usage.get("cache_creation") or {}
                    cw5 = int(cc.get("ephemeral_5m_input_tokens", 0))
                    cw1 = int(cc.get("ephemeral_1h_input_tokens", 0))
                    if cw5 == 0 and cw1 == 0:
                        cw5 = int(usage.get("cache_creation_input_tokens", 0))
                    total_tokens += it + ot + cr + cw5 + cw1
                    cost += (
                        it * r["input"]
                        + ot * r["output"]
                        + cr * r["cache_r"]
                        + cw5 * r["cache_w5"]
                        + cw1 * r["cache_w1"]
                    )
    except Exception:
        return None

    if msg_count < 2:
        return None

    # Dominant model
    if models_seen:
        model_full = max(models_seen, key=lambda k: models_seen[k])
    else:
        model_full = ""
    tag = short_model(model_full)

    # Session display name
    if custom_title:
        name = custom_title
        has_name = True
    elif first_user:
        name = first_user.split("\n")[0][:60]
        has_name = False
    else:
        name = "(unnamed session)"
        has_name = False

    # Build preview text
    preview_text = _render_preview_text(preview_items)

    blob = " ".join([name, project_name, first_user]).lower()

    return Session(
        session_id=session_id,
        name=name,
        has_custom_name=has_name,
        project=project_name,
        project_dir=project_enc,
        cost=cost,
        model=tag,
        model_full=model_full,
        msg_count=msg_count,
        total_tokens=total_tokens,
        mtime=mtime,
        file_path=jf,
        preview_text=preview_text,
        first_user=first_user,
        search_blob=blob,
    )


def _render_preview_text(items: list[tuple[str, str]]) -> str:
    """Return a Rich-markup string (we render it via Static with markup=True)."""
    if not items:
        return "[#6C7086 italic](empty conversation)[/]"
    lines: list[str] = []
    for role, text in items:
        clean = text.replace("\n", " ").strip()
        if len(clean) > 200:
            clean = clean[:200] + "…"
        # escape rich markup in user text — avoid injection from [key] patterns
        clean = clean.replace("[", "\\[")
        if role == "user":
            lines.append(f"[#89B4FA bold]user[/]    [#CDD6F4]{clean}[/]")
        else:
            lines.append(f"[#F9E2AF bold]claude[/]  [#A6ADC8]{clean}[/]")
    return "\n\n".join(lines)


# ── Demo sessions (for --demo and if no real data found) ─────────────

def demo_sessions() -> list[Session]:
    now = time.time()
    rows = [
        ("auth-refactor",       0.41, "sonnet", "claude-sonnet-4-5",  42, 18_200, now - 2 * 3600,
         "Move session auth into a shared middleware so both the web and CLI flows share identity.",
         "I'll split this into four steps: extract the shared auth, wire the CLI, add tests, and document the flow. Starting with the extraction now."),
        ("fix-race-condition",  1.24, "opus",   "claude-opus-4-7",    88, 41_400, now - 3 * 3600,
         "The background worker sometimes overwrites newer records when two users edit at the same time.",
         "Root cause is the read-modify-write without a version check. Let me add optimistic concurrency via updated_at and retries."),
        ("drizzle-migration",   0.10, "haiku",  "claude-haiku-4-5",   12,  6_800, now - 26 * 3600,
         "Migrate the users table to Drizzle — keep the existing column names and add a created_at trigger.",
         "Generated the schema + migration SQL. Need one manual step for the trigger since Drizzle kit doesn't emit it directly."),
        ("mcp-postgres-setup",  0.62, "sonnet", "claude-sonnet-4-5",  31, 22_100, now - 26 * 3600,
         "Wire the MCP postgres server into my dev environment so Claude can query staging read-replicas.",
         "Done. Connection URI via env var, read-only role, and I've added a sample query so you can confirm it's alive."),
        ("fix-payment-webhook", 2.07, "opus",   "claude-opus-4-7",   156, 93_700, now - 2 * 86400,
         "Stripe webhook is 500-ing when the invoice_id is missing, and we're losing events.",
         "Fixed: treated missing invoice_id as a handled no-op, added an idempotency key, and backfilled the 6 lost events from Stripe's dashboard."),
        ("session-tree-feature", 0.53, "sonnet", "claude-sonnet-4-5",  48, 19_200, now - 3 * 86400,
         "Add a session-tree view to claude-picker so forks show as a branch diagram.",
         "Built. Each child session links via parentSessionId; the tree renders with ascii branches and highlights the currently-active path."),
    ]
    sessions = []
    for name, cost, tag, full, mc, tok, mt, user_q, ai_r in rows:
        pv = _render_preview_text([("user", user_q), ("claude", ai_r)])
        sessions.append(Session(
            session_id=f"demo-{name}",
            name=name,
            has_custom_name=True,
            project="demo-project",
            project_dir="demo",
            cost=cost,
            model=tag,
            model_full=full,
            msg_count=mc,
            total_tokens=tok,
            mtime=mt,
            file_path="",
            preview_text=pv,
            first_user=user_q,
            search_blob=f"{name} demo-project {user_q}".lower(),
        ))
    return sessions


# ── The Textual app ───────────────────────────────────────────────────

class ClaudePickerApp(App):
    """Session picker, reimagined in Textual."""

    CSS_PATH = "session-picker-tui.tcss"
    TITLE = "claude-picker"
    SUB_TITLE = "Textual prototype"

    BINDINGS = [
        Binding("up,k",       "cursor_up",      "Up",        show=False),
        Binding("down,j",     "cursor_down",    "Down",      show=False),
        Binding("pageup",     "page_up",        "PgUp",      show=False),
        Binding("pagedown",   "page_down",      "PgDn",      show=False),
        Binding("home",       "first",          "Top",       show=False),
        Binding("end",        "last",           "End",       show=False),
        Binding("enter",      "resume",         "Resume",    priority=True),
        Binding("ctrl+b",     "bookmark",       "Bookmark"),
        Binding("ctrl+e",     "export",         "Export"),
        Binding("ctrl+d",     "delete",         "Delete"),
        Binding("escape",     "clear_filter",   "Reset"),
        Binding("q,ctrl+c",   "quit",           "Quit"),
    ]

    # Reactive state ------------------------------------------------------
    filter_text: reactive[str] = reactive("")
    selected_index: reactive[int] = reactive(0)

    def __init__(self, sessions: list[Session], demo: bool = False) -> None:
        super().__init__()
        self._all_sessions: list[Session] = sessions
        self._filtered: list[Session] = list(sessions)
        self._demo = demo
        self._chosen: Optional[Session] = None
        self._toast_timer = None

    # ── Layout ────────────────────────────────────────────────────────

    def compose(self) -> ComposeResult:
        with Vertical(id="root"):
            with Horizontal(id="title-bar"):
                yield Label(" ~ ", id="title-tilde")
                yield Label("claude-picker", id="title-name")
                yield Label(
                    ("  demo mode" if self._demo else f"  {len(self._all_sessions)} sessions across all projects"),
                    id="title-mode",
                )
            with Horizontal(id="main"):
                with Vertical(id="list-pane"):
                    yield Input(placeholder="type to filter sessions...", id="filter")
                    with Horizontal(id="counter-row"):
                        yield Label("", id="counter-left")
                        yield Label("", id="counter-right")
                    yield DataTable(id="sessions", cursor_type="row", zebra_stripes=False)
                    yield Static("no sessions match that filter", id="empty")
                with Vertical(id="preview-pane"):
                    with Horizontal(id="preview-header"):
                        yield Label("PREVIEW", id="preview-label")
                        yield Label("", id="preview-id")
                    yield Label("", id="preview-title")
                    yield Label("", id="preview-meta")
                    yield Label("", id="preview-divider")
                    yield Static("", id="preview-content", markup=True)
                    yield Label("", id="preview-stats")
            yield Static("", id="toast")
        yield Footer()

    # ── Mount / wire-up ────────────────────────────────────────────────

    def on_mount(self) -> None:
        table = self.query_one("#sessions", DataTable)
        # Columns — widths chosen to match the website mockup's alignment.
        table.add_column("name",   key="name",   width=26)
        table.add_column("cost",   key="cost",   width=7)
        table.add_column("model",  key="model",  width=8)
        table.add_column("age",    key="age",    width=5)
        table.show_header = False
        self._populate_table(self._filtered)
        self._update_counter()
        self._update_preview()
        # Focus the filter input so typing works immediately.
        self.set_focus(self.query_one("#filter", Input))

    # ── Table population ───────────────────────────────────────────────

    def _row_for(self, s: Session, selected: bool) -> list[Any]:
        pointer = "▸" if selected else " "
        name_color = "#A6E3A1" if selected else ("#CDD6F4" if s.has_custom_name else "#A6ADC8")
        name_style = "bold" if selected else ("bold" if s.has_custom_name else "italic")
        # Truncate name sensibly
        name = s.name
        if len(name) > 24:
            name = name[:23] + "…"
        name_cell = Text.from_markup(
            f"[{name_color}][{name_style}]{pointer} {name}[/][/]"
        )
        cost_cell = Text.from_markup(f"[#A6E3A1]{format_cost(s.cost)}[/]", justify="right")
        mc = model_color(s.model)
        model_cell = Text.from_markup(f"[{mc}]{s.model}[/]")
        age_cell = Text.from_markup(f"[#6C7086]{human_age(s.mtime)}[/]", justify="right")
        return [name_cell, cost_cell, model_cell, age_cell]

    def _populate_table(self, sessions: list[Session]) -> None:
        table = self.query_one("#sessions", DataTable)
        table.clear()
        empty = self.query_one("#empty", Static)
        if not sessions:
            empty.add_class("-visible")
            table.add_class("-hidden")
            return
        empty.remove_class("-visible")
        table.remove_class("-hidden")
        for i, s in enumerate(sessions):
            table.add_row(*self._row_for(s, selected=(i == 0)), key=s.session_id)
        if sessions:
            table.move_cursor(row=0)

    def _update_counter(self) -> None:
        total = len(self._all_sessions)
        shown = len(self._filtered)
        left = self.query_one("#counter-left", Label)
        right = self.query_one("#counter-right", Label)
        if self.filter_text:
            left.update(f'filter: "{self.filter_text}"')
        else:
            left.update("")
        right.update(f"{shown} / {total}")

    # ── Preview pane ───────────────────────────────────────────────────

    def _update_preview(self) -> None:
        content = self.query_one("#preview-content", Static)
        title = self.query_one("#preview-title", Label)
        meta = self.query_one("#preview-meta", Label)
        divider = self.query_one("#preview-divider", Label)
        stats = self.query_one("#preview-stats", Label)
        pid = self.query_one("#preview-id", Label)

        if not self._filtered:
            title.update(Text("no session selected", style="#6C7086 italic"))
            meta.update("")
            divider.update("")
            content.update("")
            stats.update("")
            pid.update("")
            return

        i = self._clamp_index(self.selected_index)
        s = self._filtered[i]

        title_color = "#A6E3A1" if s.has_custom_name else "#A6ADC8"
        title_style = "bold" if s.has_custom_name else "italic"
        title.update(Text.from_markup(f"[{title_color}][{title_style}]{s.name}[/][/]"))

        meta.update(Text.from_markup(
            f"[#6C7086]{s.project}[/]  [#45475A]·[/]  "
            f"[#A6ADC8]{datetime.fromtimestamp(s.mtime).strftime('%b %d %H:%M')}[/]"
        ))
        divider.update(Text("─" * 60, style="#313244"))

        pid.update(Text.from_markup(f"[#A6ADC8]{s.session_id[:8].upper()}[/]"))

        # Fade transition
        content.styles.opacity = 0.0
        content.update(s.preview_text)
        content.styles.animate("opacity", value=1.0, duration=0.25)

        mc = model_color(s.model)
        stats.update(Text.from_markup(
            f"[#6C7086]msgs[/] [#CDD6F4]{s.msg_count}[/]   "
            f"[#6C7086]tokens[/] [#CDD6F4]{format_tokens(s.total_tokens)}[/]   "
            f"[#6C7086]model[/] [{mc}]{s.model}[/]   "
            f"[#6C7086]cost[/] [#A6E3A1]{format_cost(s.cost)}[/]"
        ))

    def _clamp_index(self, i: int) -> int:
        if not self._filtered:
            return 0
        return max(0, min(i, len(self._filtered) - 1))

    # ── Filtering ──────────────────────────────────────────────────────

    def on_input_changed(self, event: Input.Changed) -> None:
        if event.input.id != "filter":
            return
        self.filter_text = event.value
        q = event.value.strip().lower()
        if not q:
            self._filtered = list(self._all_sessions)
        else:
            tokens = q.split()
            self._filtered = [
                s for s in self._all_sessions
                if all(tok in s.search_blob for tok in tokens)
            ]
        self._populate_table(self._filtered)
        self.selected_index = 0
        self._update_counter()
        self._update_preview()

    # ── DataTable cursor events ────────────────────────────────────────

    def on_data_table_row_highlighted(self, event: DataTable.RowHighlighted) -> None:
        if not self._filtered:
            return
        table = event.data_table
        row_idx = table.cursor_row
        # Redraw prev row without pointer, current row with pointer.
        # (Textual's DataTable style handles the background; we repaint text.)
        prev = self.selected_index
        self.selected_index = row_idx
        self._repaint_row(prev)
        self._repaint_row(row_idx)
        self._update_preview()

    def _repaint_row(self, idx: int) -> None:
        if idx < 0 or idx >= len(self._filtered):
            return
        table = self.query_one("#sessions", DataTable)
        s = self._filtered[idx]
        selected = (idx == self.selected_index)
        cells = self._row_for(s, selected=selected)
        key = s.session_id
        try:
            for col_idx, col_key in enumerate(("name", "cost", "model", "age")):
                table.update_cell(row_key=key, column_key=col_key, value=cells[col_idx])
        except Exception:
            pass  # happens during clear / rebuild; safe to ignore

    # ── Actions ────────────────────────────────────────────────────────

    def action_cursor_up(self) -> None:
        table = self.query_one("#sessions", DataTable)
        table.action_cursor_up()

    def action_cursor_down(self) -> None:
        table = self.query_one("#sessions", DataTable)
        table.action_cursor_down()

    def action_page_up(self) -> None:
        table = self.query_one("#sessions", DataTable)
        table.action_page_up()

    def action_page_down(self) -> None:
        table = self.query_one("#sessions", DataTable)
        table.action_page_down()

    def action_first(self) -> None:
        table = self.query_one("#sessions", DataTable)
        table.move_cursor(row=0)

    def action_last(self) -> None:
        table = self.query_one("#sessions", DataTable)
        if self._filtered:
            table.move_cursor(row=len(self._filtered) - 1)

    def action_resume(self) -> None:
        if not self._filtered:
            self._show_toast("nothing to resume")
            return
        s = self._filtered[self._clamp_index(self.selected_index)]
        self._chosen = s
        self.exit(result=s)

    def action_clear_filter(self) -> None:
        inp = self.query_one("#filter", Input)
        if inp.value:
            inp.value = ""
            # on_input_changed will rebuild
        else:
            # already clear — re-focus the input as a gentle reset gesture
            self.set_focus(inp)

    def action_bookmark(self) -> None:
        self._show_toast("bookmarks — coming soon in v2")

    def action_export(self) -> None:
        self._show_toast("export — coming soon in v2")

    def action_delete(self) -> None:
        self._show_toast("delete — coming soon in v2")

    # ── Toast overlay ──────────────────────────────────────────────────

    def _show_toast(self, message: str) -> None:
        toast = self.query_one("#toast", Static)
        toast.update(Text(message, style="#CDD6F4 bold"))
        toast.add_class("-visible")
        if self._toast_timer is not None:
            self._toast_timer.stop()
        self._toast_timer = self.set_timer(1.6, self._hide_toast)

    def _hide_toast(self) -> None:
        toast = self.query_one("#toast", Static)
        toast.remove_class("-visible")


# ── Entry point ──────────────────────────────────────────────────────

def main() -> None:
    demo = "--demo" in sys.argv
    projects_dir = os.path.expanduser("~/.claude/projects")
    if demo:
        sessions = demo_sessions()
    else:
        sessions = load_sessions(projects_dir)
        if not sessions:
            # Fall back to demo data if user has no real sessions
            print(
                "No Claude Code sessions found at ~/.claude/projects/.\n"
                "Falling back to demo data so you can see the UI.\n"
                "Run `claude` somewhere real to populate real sessions.\n",
                file=sys.stderr,
            )
            sessions = demo_sessions()
            demo = True

    app = ClaudePickerApp(sessions, demo=demo)
    result = app.run()

    # After the app exits, print the chosen session id to stderr.
    # A real launcher would now `exec claude --resume <id>`.
    if isinstance(result, Session):
        print(f"\nchosen session: {result.session_id}", file=sys.stderr)
        print(f"  project: {result.project}", file=sys.stderr)
        print(f"  file:    {result.file_path or '(demo)'}", file=sys.stderr)
        print(f"\n  (real launcher would run: claude --resume {result.session_id})", file=sys.stderr)


if __name__ == "__main__":
    main()

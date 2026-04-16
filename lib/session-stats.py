#!/usr/bin/env python3
"""Session stats dashboard for claude-picker.

Renders a rich terminal dashboard with KPI cards (with sparklines),
per-project horizontal bars, a 30-day activity timeline, and a footer.
"""

import json, glob, os, shutil, sys, time
from datetime import datetime, timedelta, date
from collections import defaultdict

# ── Catppuccin Mocha palette ──────────────────────────────────────────────
R  = '\033[0m'
B  = '\033[1m'
D  = '\033[2m'
I  = '\033[3m'

TEXT   = '\033[38;5;253m'   # #CDD6F4
SUB    = '\033[38;5;244m'   # #6C7086 muted
LINE   = '\033[38;5;238m'   # #45475A dividers
MAUVE  = '\033[38;5;141m'   # #CBA6F7
GREEN  = '\033[38;5;114m'   # #A6E3A1
YELLOW = '\033[38;5;222m'   # #F9E2AF
BLUE   = '\033[38;5;111m'   # #89B4FA
PEACH  = '\033[38;5;215m'   # #FAB387
RED    = '\033[38;5;210m'   # #F38BA8
TEAL   = '\033[38;5;116m'   # #94E2D5
PINK   = '\033[38;5;217m'   # #F5C2E7

SPARK_CHARS = '▁▂▃▄▅▆▇█'
BAR_FULL    = '█'
BAR_EMPTY   = '░'

# ── Pricing per model (USD per token) ─────────────────────────────────────
# Claude Code logs per-message usage AND the model in every assistant entry.
# We look up the right price for each message so mixed-model sessions
# (Opus 4.7 today, Opus 4.6 earlier, maybe Sonnet 4.5) stay accurate.

def _rates(inp, out, cw5, cw1, cr):
    return dict(
        input=inp / 1_000_000, output=out / 1_000_000,
        cache_w5=cw5 / 1_000_000, cache_w1=cw1 / 1_000_000,
        cache_r=cr / 1_000_000,
    )

MODEL_PRICES = [
    # Opus family — $15 / $75 per 1M, 5m cache $18.75, 1h $30, read $1.50
    ('claude-opus-4',     _rates(15.00, 75.00, 18.75, 30.00, 1.50)),
    ('claude-3-opus',     _rates(15.00, 75.00, 18.75, 30.00, 1.50)),
    # Sonnet family — $3 / $15, cache $3.75 / $6, read $0.30
    ('claude-sonnet-4',   _rates( 3.00, 15.00,  3.75,  6.00, 0.30)),
    ('claude-3-7-sonnet', _rates( 3.00, 15.00,  3.75,  6.00, 0.30)),
    ('claude-3-5-sonnet', _rates( 3.00, 15.00,  3.75,  6.00, 0.30)),
    # Haiku family — $0.80 / $4, cache $1 / $1.60, read $0.08
    ('claude-haiku-4',    _rates( 0.80,  4.00,  1.00,  1.60, 0.08)),
    ('claude-3-5-haiku',  _rates( 0.80,  4.00,  1.00,  1.60, 0.08)),
]
# Fallback if a model isn't recognised — use Opus rates (most conservative)
FALLBACK_RATES = MODEL_PRICES[0][1]

def rates_for(model):
    if not model or model == '<synthetic>':
        return None   # skip internal messages
    for prefix, rates in MODEL_PRICES:
        if model.startswith(prefix):
            return rates
    return FALLBACK_RATES

# ── Helpers ───────────────────────────────────────────────────────────────

def term_width():
    try:
        return shutil.get_terminal_size((100, 30)).columns
    except Exception:
        return 100

# Cap dashboard width so it doesn't stretch ugly on ultra-wide terminals.
MAX_W = 120
MIN_W = 90

def sparkline(values, width=8):
    """Render a sparkline. Zero values render as `·` so sparse data still fills the card visually."""
    if not values:
        return '·' * width
    if len(values) > width:
        step = len(values) / width
        sampled = [values[min(int(i * step), len(values) - 1)] for i in range(width)]
    else:
        sampled = list(values) + [0] * (width - len(values))
    nonzero = [v for v in sampled if v > 0]
    if not nonzero:
        return '·' * width
    lo, hi = min(nonzero), max(nonzero)
    rng = (hi - lo) if hi > lo else 1
    out = ''
    for v in sampled:
        if v <= 0:
            out += '·'
            continue
        # Lift the minimum visible bar so lone peaks read clearly
        idx = max(2, int(((v - lo) / rng) * (len(SPARK_CHARS) - 1)))
        out += SPARK_CHARS[idx]
    return out

def format_number(n):
    if n >= 1_000_000:
        return f'{n / 1_000_000:.1f}M'
    if n >= 1_000:
        return f'{n / 1_000:.1f}k'
    return str(n)

def format_cost(c):
    if c >= 1000:
        return f'${c:,.0f}'
    return f'${c:,.2f}'

def format_tokens(t):
    if t >= 1_000_000:
        return f'{t / 1_000_000:.1f}M'
    if t >= 1_000:
        return f'{t / 1_000:.0f}k'
    return str(t)

def project_color(i):
    palette = [GREEN, TEAL, BLUE, YELLOW, PEACH, PINK, MAUVE]
    return palette[i % len(palette)]

def strip_ansi(s):
    import re
    return re.sub(r'\033\[[0-9;]*m', '', s)

def pad_visible(s, width):
    visible = len(strip_ansi(s))
    return s + ' ' * max(0, width - visible)

# ── Data collection ──────────────────────────────────────────────────────

projects_dir = os.path.expanduser('~/.claude/projects')
sessions_dir = os.path.expanduser('~/.claude/sessions')
now_dt = datetime.now()
today = now_dt.date()

meta_cwds = {}
for mf in glob.glob(os.path.join(sessions_dir, '*.json')):
    try:
        data = json.load(open(mf))
        sid = data.get('sessionId', '')
        cwd = data.get('cwd', '')
        if sid and cwd:
            meta_cwds[sid] = cwd
    except Exception:
        pass

total_sessions = 0
total_input_tokens = 0      # actual input (non-cache)
total_output_tokens = 0     # actual output
total_cache_read = 0        # cache reads (counted as cheap input)
total_cache_write_5 = 0     # cache writes — 5m ephemeral
total_cache_write_1 = 0     # cache writes — 1h ephemeral
total_cost = 0.0
total_size = 0
named_count = 0
unnamed_count = 0

# Per-model cost breakdown (for an optional summary line)
model_cost = defaultdict(float)

project_data = defaultdict(lambda: {
    'sessions': 0, 'input': 0, 'output': 0,
    'cache_read': 0, 'cache_write_5': 0, 'cache_write_1': 0,
    'cost': 0.0,
})

# Daily buckets for last 30 days
daily = defaultdict(lambda: {'sessions': 0, 'tokens': 0, 'cost': 0.0})
thirty_days_ago = today - timedelta(days=29)

noise_prefixes = (
    '<local-command', '<command-name>', '<bash-',
    '<system-reminder>', '[Request inter', '<command-message>',
)

if not os.path.isdir(projects_dir):
    print(f'\n  {SUB}no sessions found at {projects_dir}{R}\n')
    sys.exit(0)

for d in os.listdir(projects_dir):
    full = os.path.join(projects_dir, d)
    if not os.path.isdir(full):
        continue
    jsonl_files = glob.glob(os.path.join(full, '*.jsonl'))
    if not jsonl_files:
        continue

    project_name = None
    for jf in jsonl_files:
        try:
            with open(jf) as fh:
                for line in fh:
                    data = json.loads(line.strip())
                    sid = data.get('sessionId', '')
                    if sid and sid in meta_cwds:
                        cand = meta_cwds[sid]
                        if os.path.isdir(cand):
                            project_name = os.path.basename(cand)
                            break
                    break
        except Exception:
            pass
        if project_name:
            break

    if not project_name:
        project_name = d.split('-')[-1] if '-' in d else d

    for jf in jsonl_files:
        file_size = os.path.getsize(jf)
        mod_ts = os.path.getmtime(jf)
        mod_date = datetime.fromtimestamp(mod_ts).date()

        is_claude = True
        try:
            with open(jf) as fh:
                for line in fh:
                    data = json.loads(line.strip())
                    ep = data.get('entrypoint', '')
                    if ep and ep not in ('cli', 'sdk-cli'):
                        is_claude = False
                        break
                    if ep in ('cli', 'sdk-cli'):
                        break
        except Exception:
            pass
        if not is_claude:
            continue

        session_name = None
        msg_count = 0
        s_input = 0
        s_output = 0
        s_cache_read = 0
        s_cache_write_5 = 0
        s_cache_write_1 = 0
        s_cost = 0.0

        try:
            with open(jf) as fh:
                for line in fh:
                    data = json.loads(line.strip())

                    if data.get('type') == 'custom-title' and data.get('customTitle'):
                        session_name = data['customTitle']

                    if data.get('type') in ('user', 'assistant'):
                        msg = data.get('message', {}) or {}
                        role = msg.get('role')
                        if role not in ('user', 'assistant'):
                            continue
                        msg_count += 1

                        # Read real token usage + model from the assistant message.
                        # Claude Code logs per-message usage with cache split and model id.
                        if role == 'assistant':
                            model = msg.get('model') or ''
                            rates = rates_for(model)
                            if rates is None:
                                continue   # skip <synthetic> or unknown-zero entries

                            usage = msg.get('usage') or {}
                            it = int(usage.get('input_tokens', 0))
                            ot = int(usage.get('output_tokens', 0))
                            cr = int(usage.get('cache_read_input_tokens', 0))
                            cc = usage.get('cache_creation', {}) or {}
                            cw5 = int(cc.get('ephemeral_5m_input_tokens', 0))
                            cw1 = int(cc.get('ephemeral_1h_input_tokens', 0))
                            if cw5 == 0 and cw1 == 0:
                                cw5 = int(usage.get('cache_creation_input_tokens', 0))

                            s_input          += it
                            s_output         += ot
                            s_cache_read     += cr
                            s_cache_write_5  += cw5
                            s_cache_write_1  += cw1

                            msg_cost = (
                                it  * rates['input']    +
                                ot  * rates['output']   +
                                cr  * rates['cache_r']  +
                                cw5 * rates['cache_w5'] +
                                cw1 * rates['cache_w1']
                            )
                            s_cost += msg_cost
                            model_cost[model] += msg_cost
        except Exception:
            pass

        if msg_count < 2:
            continue

        total_size += file_size
        total_sessions += 1
        total_input_tokens    += s_input
        total_output_tokens   += s_output
        total_cache_read      += s_cache_read
        total_cache_write_5   += s_cache_write_5
        total_cache_write_1   += s_cache_write_1
        total_cost            += s_cost

        if session_name:
            named_count += 1
        else:
            unnamed_count += 1

        project_data[project_name]['sessions']      += 1
        project_data[project_name]['input']         += s_input
        project_data[project_name]['output']        += s_output
        project_data[project_name]['cache_read']    += s_cache_read
        project_data[project_name]['cache_write_5'] += s_cache_write_5
        project_data[project_name]['cache_write_1'] += s_cache_write_1
        project_data[project_name]['cost']          += s_cost

        if mod_date >= thirty_days_ago:
            daily[mod_date]['sessions'] += 1
            daily[mod_date]['tokens']   += s_input + s_output + s_cache_read + s_cache_write_5 + s_cache_write_1
            daily[mod_date]['cost']     += s_cost

# ── Build 30-day series ──

days_series = [thirty_days_ago + timedelta(days=i) for i in range(30)]
daily_sessions = [daily[d]['sessions'] for d in days_series]
daily_tokens   = [daily[d]['tokens']   for d in days_series]
daily_cost     = [daily[d]['cost']     for d in days_series]

# Use full 30-day window for sparklines so trends are visible even on sparse data.
spark_sessions = daily_sessions
spark_tokens   = daily_tokens
spark_cost     = daily_cost

sum_sessions_30 = sum(daily_sessions)
sum_tokens_30   = sum(daily_tokens)
sum_cost_30     = sum(daily_cost)
days_with_activity = sum(1 for c in daily_cost if c > 0) or 1
avg_cost_per_day = sum_cost_30 / 30.0

total_tokens = (
    total_input_tokens + total_output_tokens +
    total_cache_read + total_cache_write_5 + total_cache_write_1
)
# total_cost already accumulated accurately per-message

# ── Render ──

W = min(MAX_W, max(MIN_W, term_width() - 2))
card_w = (W - 8) // 3   # 3 cards + 2 gaps + 2 margin
# Right-side metric column used by per-project rows: "$xx.xx  ·  xx.xM tok  ·  xxx ses"
RIGHT_COL_W = 35

def rule(label, w=W, color=SUB):
    inner = f' {label} ' if label else ''
    dashes = '─' * max(0, w - len(strip_ansi(inner)))
    return f'{color}{D}── {label} ' + '─' * max(0, w - len(label) - 5) + f'{R}'

def boxed_kpi(label_color, label, big_value, big_color, spark_color, spark_vals, subtitle):
    """Four-line KPI card: top rule, big+spark, subtitle, bottom rule. All lines pad to card_w."""
    lines = []

    # Top rule: "╭─ label ──────"  visible width == card_w
    top = f'{LINE}{D}╭─ {R}{label_color}{label}{R} {LINE}{D}'
    dashes_top = '─' * max(0, card_w - (3 + len(label) + 1))
    top += dashes_top + R
    lines.append(pad_visible(top, card_w))

    # Middle: "  69k   ▁▁▇█▄"  — big value, gap, sparkline
    prefix = '  '
    gap = '   '
    spark_room = max(4, card_w - len(prefix) - len(big_value) - len(gap) - 1)
    spark = sparkline(spark_vals, width=spark_room)
    mid = f'{prefix}{big_color}{B}{big_value}{R}{gap}{spark_color}{spark}{R}'
    lines.append(pad_visible(mid, card_w))

    # Subtitle line — truncate if too long
    sub_raw = subtitle
    max_sub = card_w - 3
    if len(sub_raw) > max_sub:
        sub_raw = sub_raw[:max_sub - 1] + '…'
    sub_line = f'  {SUB}{sub_raw}{R}'
    lines.append(pad_visible(sub_line, card_w))

    # Bottom rule: "╰───────"  visible width == card_w
    bot = f'{LINE}{D}╰' + '─' * (card_w - 1) + f'{R}'
    lines.append(pad_visible(bot, card_w))

    return lines

print()
# Header
left = f'  {MAUVE}{B}claude-picker --stats{R}'
right = f'{SUB}last 30 days · all projects{R}'
pad_w = W - len(strip_ansi(left)) - len(strip_ansi(right))
print(left + ' ' * max(1, pad_w) + right)
print()

# KPI row
input_mtok  = total_input_tokens  / 1_000_000
output_mtok = total_output_tokens / 1_000_000

kpi1 = boxed_kpi(
    label_color=SUB,
    label='tokens',
    big_value=format_tokens(total_tokens),
    big_color=TEXT,
    spark_color=TEAL,
    spark_vals=spark_tokens or [0],
    subtitle=f'{format_tokens(total_input_tokens + total_cache_read + total_cache_write_5 + total_cache_write_1)} in · {format_tokens(total_output_tokens)} out',
)
kpi2 = boxed_kpi(
    label_color=SUB,
    label='cost',
    big_value=format_cost(total_cost),
    big_color=GREEN,
    spark_color=GREEN,
    spark_vals=spark_cost or [0],
    subtitle=f'avg {format_cost(avg_cost_per_day)} / day',
)
kpi3 = boxed_kpi(
    label_color=SUB,
    label='sessions',
    big_value=str(total_sessions),
    big_color=YELLOW,
    spark_color=YELLOW,
    spark_vals=spark_sessions or [0],
    subtitle=f'{named_count} named · {unnamed_count} unnamed',
)

for i in range(max(len(kpi1), len(kpi2), len(kpi3))):
    row = '  '
    row += kpi1[i] if i < len(kpi1) else ' ' * card_w
    row += '  '
    row += kpi2[i] if i < len(kpi2) else ' ' * card_w
    row += '  '
    row += kpi3[i] if i < len(kpi3) else ' ' * card_w
    print(row)

print()

# Per project
if project_data:
    print(f'  {SUB}{D}── per project {"─" * (W - 20)}{R}')
    print()

    sorted_proj = sorted(project_data.items(), key=lambda x: x[1]['cost'], reverse=True)[:8]
    max_cost = sorted_proj[0][1]['cost'] if sorted_proj else 1

    name_w = min(18, max((len(n) for n, _ in sorted_proj), default=8) + 2)
    # Layout: "  " + name(name_w) + "  " + bar(bar_w) + "  " + right(RIGHT_COL_W)
    # Total visible must be ≤ W. Give bar whatever's left.
    bar_w = max(10, W - name_w - RIGHT_COL_W - 6)

    for i, (name, info) in enumerate(sorted_proj):
        color = project_color(i)
        if max_cost > 0 and info['cost'] > 0:
            filled = max(1, int((info['cost'] / max_cost) * bar_w))
        else:
            filled = 0
        bar = color + BAR_FULL * filled + R + LINE + BAR_EMPTY * (bar_w - filled) + R
        # Total billable tokens for this project
        toks = info['input'] + info['output'] + info['cache_read'] + info['cache_write_5'] + info['cache_write_1']
        # Right-side column — keep total visible width == RIGHT_COL_W (35 chars)
        right = f'{format_cost(info["cost"]):>7}  ·  {format_tokens(toks):>5} tok  ·  {info["sessions"]:>3} ses'
        print(f'  {color}{B}{name:<{name_w}}{R}  {bar}  {SUB}{right}{R}')
    print()

# Activity timeline (30 days)
print(f'  {SUB}{D}── activity (30d) ' + '─' * max(0, W - 20) + f'{R}')
print()

max_day = max(daily_sessions) if any(daily_sessions) else 1

# 3 cols per day (bar + 2 spaces) — gives 90-col bar area for 30 days
SLOT = 3
BAR_AREA = 30 * SLOT  # 90 visible cols for bars

# 5 labels spaced evenly, plus "today" at the end = 6 labels.
# Positions chosen so labels never collide: each label takes 6 chars, gap >= 6.
label_indices = [0, 6, 13, 20, 29]    # 5 anchor labels ~ every ~6-7 days
dynamic_labels = [days_series[i].strftime('%b %d') for i in label_indices]

def day_bar(count, is_today, is_spike):
    if count == 0:
        return f'{LINE}{D}·{R}'
    # Stretch the height so even low-activity days are visible (min ▃)
    if max_day == 1:
        idx = 4  # single-activity day renders as a solid mid-bar
    else:
        norm = (count / max_day)
        idx = max(2, int(norm * (len(SPARK_CHARS) - 1)))
    ch = SPARK_CHARS[idx]
    if is_today:
        return f'{GREEN}{B}{ch}{R}'
    if is_spike:
        return f'{RED}{ch}{R}'
    return f'{MAUVE}{ch}{R}'

avg_sessions = (sum_sessions_30 / 30) if sum_sessions_30 else 0
spike_threshold = max(2, avg_sessions * 2.2)

# Center the bar area horizontally within W
left_pad = max(4, (W - BAR_AREA) // 2)

# Bar line
bar_line = ' ' * left_pad
for i, d in enumerate(days_series):
    count = daily_sessions[i]
    is_today = (d == today)
    is_spike = (count >= spike_threshold and count > 0)
    bar_line += day_bar(count, is_today, is_spike) + '  '
print(bar_line.rstrip())

# Label line — each anchor day's column is day_idx * SLOT
label_cells = [' '] * BAR_AREA
for li, day_idx in enumerate(label_indices):
    lbl = dynamic_labels[li]
    start = day_idx * SLOT
    # Right-align the final label so "Apr 16" doesn't overflow past BAR_AREA
    if start + len(lbl) > BAR_AREA:
        start = BAR_AREA - len(lbl)
    for k, ch in enumerate(lbl):
        if 0 <= start + k < BAR_AREA:
            label_cells[start + k] = ch
label_line = ' ' * left_pad + f'{SUB}' + ''.join(label_cells) + f'{R}'
print(label_line)

# Annotation line — today arrow only (spike annotations add visual noise with real data)
if daily_sessions and daily_sessions[-1] > 0:
    ann_cells = [' '] * BAR_AREA
    arrow = '↑ today'
    start_today = BAR_AREA - len(arrow)
    for k, ch in enumerate(arrow):
        if 0 <= start_today + k < BAR_AREA:
            ann_cells[start_today + k] = ch
    print(' ' * left_pad + f'{GREEN}' + ''.join(ann_cells) + f'{R}')

print()

# Per-model cost breakdown (shown only if more than one model was used)
if len(model_cost) > 1:
    parts = []
    for m, c in sorted(model_cost.items(), key=lambda x: -x[1]):
        short = m.replace('claude-', '').replace('-20', ' 20')
        parts.append(f'{MAUVE}{short}{R} {GREEN}{format_cost(c)}{R}')
    breakdown = f'  {SUB}by model:{R}  ' + f'{SUB} · {R}'.join(parts)
    print(breakdown)
    print()

# Footer
foot = f'  {SUB}press {R}{TEXT}q{R}{SUB} to quit  ·  press {R}{TEXT}e{R}{SUB} to export  ·  press {R}{TEXT}t{R}{SUB} to toggle days/weeks{R}'
print(foot)
print()

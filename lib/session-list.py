#!/usr/bin/env python3
"""Builds the session list for fzf.
Features: auto-naming, token/cost estimate, bookmarks, age warnings, visual hierarchy."""

import json, glob, os, time
from datetime import datetime

project_dir = os.environ.get('PROJECT_DIR', '')
sessions_meta_dir = os.environ.get('SESSIONS_META_DIR', os.path.expanduser('~/.claude/sessions'))
now = time.time()

import sys
if not project_dir or not os.path.isdir(project_dir):
    # Silently exit — main script handles "no sessions" message
    sys.exit(0)

# Catppuccin Mocha palette
R  = '\033[0m';  B  = '\033[1m';  D  = '\033[2m';  I  = '\033[3m'
CY = '\033[38;5;117m'; GN = '\033[38;5;114m'; YL = '\033[38;5;222m'
MG = '\033[38;5;176m'; DG = '\033[38;5;242m'; GR = '\033[38;5;249m'
BL = '\033[38;5;111m'; OR = '\033[38;5;215m'; RD = '\033[38;5;203m'
PE = '\033[38;5;215m'  # peach
PB = '\033[38;2;137;180;250m'  # pin blue #89B4FA

# Load bookmarks
bookmarks = set()
bookmarks_file = os.path.expanduser('~/.claude-picker/bookmarks.json')
try:
    if os.path.exists(bookmarks_file):
        bookmarks = set(json.load(open(bookmarks_file)))
except:
    pass

SEVEN_DAYS = 7 * 86400
THIRTY_DAYS = 30 * 86400

def relative_time(ts):
    diff = now - ts
    if diff < 60: return 'just now'
    elif diff < 3600: return f'{int(diff/60)}m ago'
    elif diff < 86400: return f'{int(diff/3600)}h ago'
    elif diff < 604800: return f'{int(diff/86400)}d ago'
    else: return datetime.fromtimestamp(ts).strftime('%b %d')

def time_color(ts):
    diff = now - ts
    if diff > THIRTY_DAYS: return RD
    elif diff > SEVEN_DAYS: return PE
    else: return DG

def age_indicator(ts):
    diff = now - ts
    if diff > THIRTY_DAYS: return f' {RD}\u26a0{R}'
    return ''

def estimate_tokens(char_count):
    return max(1, char_count // 4)

def cost_color(tokens):
    if tokens < 50000: return DG
    elif tokens < 200000: return YL
    else: return RD

meta_by_id = {}
for mf in glob.glob(os.path.join(sessions_meta_dir, '*.json')):
    try:
        data = json.load(open(mf))
        sid = data.get('sessionId', '')
        if sid: meta_by_id[sid] = data
    except:
        pass

bookmarked_sessions = []
named = []
unnamed = []

noise = ['<local-command', '<command-name>', '<bash-', '<system-reminder>', '[Request inter', '---', '<command-message>']

for f in sorted(glob.glob(os.path.join(project_dir, '*.jsonl')), key=os.path.getmtime, reverse=True):
    session_id = os.path.basename(f).replace('.jsonl', '')
    mod_ts = os.path.getmtime(f)
    rel_time = relative_time(mod_ts)

    # Filter: Claude CLI only
    is_claude = True
    try:
        for line in open(f):
            data = json.loads(line.strip())
            ep = data.get('entrypoint', '')
            if ep and ep not in ('cli', 'sdk-cli'):
                is_claude = False; break
            if ep in ('cli', 'sdk-cli'): break
    except:
        pass
    if not is_claude: continue

    name = None
    auto_name = None
    msg_count = 0
    total_chars = 0

    try:
        for line in open(f):
            data = json.loads(line.strip())

            if data.get('type') == 'custom-title' and data.get('customTitle'):
                name = data['customTitle'][:35]

            if data.get('type') in ('user', 'assistant') and data.get('message', {}).get('role') in ('user', 'assistant'):
                msg_count += 1
                content = data['message'].get('content', '')
                if isinstance(content, str):
                    total_chars += len(content)
                elif isinstance(content, list):
                    for item in content:
                        if isinstance(item, dict) and item.get('type') == 'text':
                            total_chars += len(item.get('text', ''))

                if not auto_name and data.get('type') == 'user':
                    text = ''
                    if isinstance(content, str):
                        text = content.strip()
                    elif isinstance(content, list):
                        for item in content:
                            if isinstance(item, dict) and item.get('type') == 'text':
                                text = item['text'].strip(); break
                    if text and len(text) > 3 and not any(n in text for n in noise):
                        auto_name = text[:50].replace('\n', ' ').strip()
    except:
        pass

    if not name:
        meta = meta_by_id.get(session_id, {})
        if meta.get('name'): name = meta['name'][:35]

    if msg_count < 2: continue

    tokens = estimate_tokens(total_chars)
    cc = cost_color(tokens)

    if tokens >= 1000:
        tok_display = f'~{tokens//1000}k'
    else:
        tok_display = f'~{tokens}'

    cost_str = ''
    if tokens > 10000:
        cost = (tokens * 0.4 * 3 + tokens * 0.6 * 15) / 1_000_000
        dollar = '$'
        if cost < 0.01: cost_str = f' {DG}<{dollar}0.01{R}'
        elif cost < 1.0: cost_str = f' {DG}~{dollar}{cost:.2f}{R}'
        else: cost_str = f' {DG}~{dollar}{cost:.2f}{R}'

    tc = time_color(mod_ts)
    age_warn = age_indicator(mod_ts)

    display_name = name if name else (auto_name[:28] if auto_name else 'session')
    entry = (rel_time, display_name, msg_count, tokens, tok_display, cc, session_id, tc, age_warn, cost_str, bool(name))

    if session_id in bookmarks:
        bookmarked_sessions.append(entry)
    elif name:
        named.append(entry)
    else:
        unnamed.append(entry)

# ── Output ──

W = 28  # name column width
hr = '\u2500' * 50
hr_short = '\u2500' * 42

print(f'  {MG}{B}+{R}   {CY}{B}New Session{R}  |  __NEW__')
print(f'  {DG}{D}  {hr}{R}  |  __SEP__')

def fmt_row(icon, ic, nm, nc, ns, tc, rel, aw, msgs, cc, tstr, cs, sid):
    return f'  {ic}{icon}{R}  {ns}{nc}{nm:<{W}s}{R} {tc}{rel:>8s}{R}{aw} {DG}{msgs:>4d} msgs{R} {cc}{tstr:>5s}{R}{cs}  |  {sid}'

if bookmarked_sessions:
    print(f'  {DG}{D}  \u2500\u2500 pinned {hr_short}{R}  |  __HDR0__')
    for rel, nm, msgs, tokens, tstr, cc, sid, tc, aw, cs, is_named in bookmarked_sessions:
        nc = GN if is_named else GR; ns = B if is_named else I
        print(fmt_row('\u25a0', PB, nm[:W], nc, ns, tc, rel, aw, msgs, cc, tstr, cs, sid))

if named:
    hr_saved = '\u2500' * 43
    print(f'  {DG}{D}  \u2500\u2500 saved {hr_saved}{R}  |  __HDR1__')
    for rel, nm, msgs, tokens, tstr, cc, sid, tc, aw, cs, is_named in named:
        print(fmt_row('\u25cf', YL, nm[:W], GN, B, tc, rel, aw, msgs, cc, tstr, cs, sid))

if unnamed:
    print(f'  {DG}{D}  \u2500\u2500 recent {hr_short}{R}  |  __HDR2__')
    for rel, display, msgs, tokens, tstr, cc, sid, tc, aw, cs, is_named in unnamed:
        print(fmt_row('\u25cb', DG, display[:W], GR, I, tc, rel, aw, msgs, cc, tstr, cs, sid))

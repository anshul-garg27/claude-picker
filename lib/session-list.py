#!/usr/bin/env python3
"""Builds the session list for fzf.
Features: auto-naming, token/cost estimate, bookmarks, age warnings, visual hierarchy."""

import json, glob, os, time
from datetime import datetime

project_dir = os.environ.get('PROJECT_DIR', '')
sessions_meta_dir = os.environ.get('SESSIONS_META_DIR', os.path.expanduser('~/.claude/sessions'))
now = time.time()

if not project_dir:
    import sys
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
        tok_display = f'~{tokens//1000}k tok'
    else:
        tok_display = f'~{tokens} tok'

    cost_str = ''
    if tokens > 10000:
        cost = (tokens * 0.4 * 3 + tokens * 0.6 * 15) / 1_000_000
        if cost < 0.01: cost_str = f' {DG}<$0.01{R}'
        elif cost < 1.0: cost_str = f' {DG}~${cost:.2f}{R}'
        else: cost_str = f' {DG}~${cost:.2f}{R}'

    tc = time_color(mod_ts)
    age_warn = age_indicator(mod_ts)

    display_name = name if name else (auto_name[:35] if auto_name else 'session')
    entry = (rel_time, display_name, msg_count, tokens, tok_display, cc, session_id, tc, age_warn, cost_str, bool(name))

    if session_id in bookmarks:
        bookmarked_sessions.append(entry)
    elif name:
        named.append(entry)
    else:
        unnamed.append(entry)

# ── Output ──

print(f'  {MG}{B}+{R}   {CY}{B}New Session{R}                                        {DG}start fresh{R}  |  __NEW__')
print(f'  {DG}{D}  \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500{R}  |  __SEP__')

if bookmarked_sessions:
    print(f'  {DG}  {D}\u2500\u2500 bookmarked \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500{R}  |  __HDR0__')
    for rel, nm, msgs, tokens, tstr, cc, sid, tc, aw, cs, is_named in bookmarked_sessions:
        nc = GN if is_named else GR
        ns = B if is_named else I
        print(f'  {PB}\u25a0{R}   {ns}{nc}{nm:<35s}{R}  {tc}{rel:>9s}{R}{aw}  {DG}{msgs:>3d} msgs{R}  {cc}{tstr:>7s}{R}{cs}  |  {sid}')

if named:
    print(f'  {DG}  {D}\u2500\u2500 saved \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500{R}  |  __HDR1__')
    for rel, nm, msgs, tokens, tstr, cc, sid, tc, aw, cs, is_named in named:
        print(f'  {YL}\u25cf{R}   {B}{GN}{nm:<35s}{R}  {tc}{rel:>9s}{R}{aw}  {DG}{msgs:>3d} msgs{R}  {cc}{tstr:>7s}{R}{cs}  |  {sid}')

if unnamed:
    print(f'  {DG}  {D}\u2500\u2500 recent \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500{R}  |  __HDR2__')
    for rel, display, msgs, tokens, tstr, cc, sid, tc, aw, cs, is_named in unnamed:
        print(f'  {DG}\u25cb{R}   {GR}{I}{display:<35s}{R}  {tc}{rel:>9s}{R}{aw}  {DG}{msgs:>3d} msgs{R}  {cc}{tstr:>7s}{R}{cs}  |  {sid}')

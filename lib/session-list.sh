#!/bin/bash
# Builds the session list for fzf
# Features: auto-naming, token/cost estimate, visual hierarchy

export PROJECT_DIR="${PROJECT_DIR:-$HOME/.claude/projects/$(echo "$PWD" | sed 's|[/_]|-|g')}"
export SESSIONS_META_DIR="${SESSIONS_META_DIR:-$HOME/.claude/sessions}"

python3 -c "
import json, glob, os, time
from datetime import datetime

project_dir = os.environ['PROJECT_DIR']
sessions_meta_dir = os.environ['SESSIONS_META_DIR']
now = time.time()

# Catppuccin Mocha palette
R  = '\033[0m';  B  = '\033[1m';  D  = '\033[2m';  I  = '\033[3m'
CY = '\033[38;5;117m'; GN = '\033[38;5;114m'; YL = '\033[38;5;222m'
MG = '\033[38;5;176m'; DG = '\033[38;5;242m'; GR = '\033[38;5;249m'
BL = '\033[38;5;111m'; OR = '\033[38;5;215m'; RD = '\033[38;5;203m'
PE = '\033[38;5;215m'  # peach

def relative_time(ts):
    diff = now - ts
    if diff < 60: return 'just now'
    elif diff < 3600: return f'{int(diff/60)}m ago'
    elif diff < 86400: return f'{int(diff/3600)}h ago'
    elif diff < 604800: return f'{int(diff/86400)}d ago'
    else: return datetime.fromtimestamp(ts).strftime('%b %d')

def estimate_tokens(char_count):
    # Rough estimate: ~4 chars per token for English
    return max(1, char_count // 4)

def format_cost(tokens):
    # Sonnet pricing: input \$3/MTok, output \$15/MTok
    # Rough estimate assuming 60% output, 40% input
    cost = (tokens * 0.4 * 3 + tokens * 0.6 * 15) / 1_000_000
    if cost < 0.01: return '<1c'
    elif cost < 1.0: return f'{cost:.0%}'.replace('%','') + 'c' if cost < 0.1 else f'\${cost:.2f}'
    else: return f'\${cost:.2f}'

def cost_color(tokens):
    if tokens < 50000: return DG     # gray — cheap
    elif tokens < 200000: return YL  # yellow — moderate
    else: return RD                  # red — expensive

meta_by_id = {}
for mf in glob.glob(os.path.join(sessions_meta_dir, '*.json')):
    try:
        data = json.load(open(mf))
        sid = data.get('sessionId', '')
        if sid: meta_by_id[sid] = data
    except: pass

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
    except: pass
    if not is_claude: continue

    # Extract: name, message count, token estimate, auto-name
    name = None
    auto_name = None
    msg_count = 0
    total_chars = 0

    try:
        for line in open(f):
            data = json.loads(line.strip())

            # Get custom title
            if data.get('type') == 'custom-title' and data.get('customTitle'):
                name = data['customTitle'][:35]

            # Count messages + estimate tokens
            if data.get('type') in ('user', 'assistant') and data.get('message', {}).get('role') in ('user', 'assistant'):
                msg_count += 1
                content = data['message'].get('content', '')
                if isinstance(content, str):
                    total_chars += len(content)
                elif isinstance(content, list):
                    for item in content:
                        if isinstance(item, dict) and item.get('type') == 'text':
                            total_chars += len(item.get('text', ''))

                # Auto-name: extract first meaningful user message
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
    except: pass

    # Check metadata for name
    if not name:
        meta = meta_by_id.get(session_id, {})
        if meta.get('name'): name = meta['name'][:35]

    if msg_count < 2: continue

    tokens = estimate_tokens(total_chars)
    token_str = f'{tokens//1000}k' if tokens >= 1000 else str(tokens)
    cc = cost_color(tokens)

    if name:
        named.append((rel_time, name, msg_count, tokens, token_str, cc, session_id))
    else:
        display_name = auto_name[:35] if auto_name else 'session'
        unnamed.append((rel_time, display_name, msg_count, tokens, token_str, cc, session_id))

# ── Output ──

# New session entry
print(f'  {MG}{B}+{R}   {CY}{B}New Session{R}                                        {DG}start fresh{R}  |  __NEW__')

if named:
    print(f'  {DG}  {D}── saved ───────────────────────────────────────────────────────{R}  |  __HDR1__')
    for rel, name, msgs, tokens, tstr, cc, sid in named:
        print(f'  {YL}●{R}   {B}{GN}{name:<35s}{R}  {DG}{rel:>9s}{R}  {DG}{msgs:>3d} msgs{R}  {cc}{tstr:>5s}{R}  |  {sid}')

if unnamed:
    print(f'  {DG}  {D}── recent ──────────────────────────────────────────────────────{R}  |  __HDR2__')
    for rel, display, msgs, tokens, tstr, cc, sid in unnamed:
        print(f'  {DG}○{R}   {GR}{I}{display:<35s}{R}  {DG}{rel:>9s}{R}  {DG}{msgs:>3d} msgs{R}  {cc}{tstr:>5s}{R}  |  {sid}')
"

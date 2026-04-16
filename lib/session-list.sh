#!/bin/bash
# Builds the session list for fzf — world class edition

export PROJECT_DIR="${PROJECT_DIR:-$HOME/.claude/projects/$(echo "$PWD" | sed 's|[/_]|-|g')}"
export SESSIONS_META_DIR="${SESSIONS_META_DIR:-$HOME/.claude/sessions}"

python3 -c "
import json, glob, os, time
from datetime import datetime

project_dir = os.environ['PROJECT_DIR']
sessions_meta_dir = os.environ['SESSIONS_META_DIR']
now = time.time()

# Refined color palette
C = {
    'r':   '\033[0m',
    'b':   '\033[1m',
    'd':   '\033[2m',
    'i':   '\033[3m',
    'wh':  '\033[97m',
    'gr':  '\033[38;5;249m',    # light gray
    'dg':  '\033[38;5;242m',    # dark gray
    'cy':  '\033[38;5;117m',    # soft cyan
    'gn':  '\033[38;5;114m',    # soft green
    'yl':  '\033[38;5;222m',    # warm yellow
    'mg':  '\033[38;5;176m',    # soft magenta/pink
    'or':  '\033[38;5;215m',    # soft orange
    'bl':  '\033[38;5;111m',    # soft blue
}

def relative_time(ts):
    diff = now - ts
    if diff < 60: return 'just now'
    elif diff < 3600: return f'{int(diff/60)}m ago'
    elif diff < 86400: return f'{int(diff/3600)}h ago'
    elif diff < 604800: return f'{int(diff/86400)}d ago'
    else: return datetime.fromtimestamp(ts).strftime('%b %d')

meta_by_id = {}
for mf in glob.glob(os.path.join(sessions_meta_dir, '*.json')):
    try:
        data = json.load(open(mf))
        sid = data.get('sessionId', '')
        if sid: meta_by_id[sid] = data
    except: pass

named = []
unnamed = []

for f in sorted(glob.glob(os.path.join(project_dir, '*.jsonl')), key=os.path.getmtime, reverse=True):
    session_id = os.path.basename(f).replace('.jsonl', '')
    mod_ts = os.path.getmtime(f)
    rel_time = relative_time(mod_ts)

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

    name = None
    try:
        for line in open(f):
            data = json.loads(line.strip())
            if data.get('type') == 'custom-title' and data.get('customTitle'):
                name = data['customTitle'][:35]
                break
    except: pass
    if not name:
        meta = meta_by_id.get(session_id, {})
        if meta.get('name'): name = meta['name'][:35]

    msg_count = 0
    try:
        for line in open(f):
            data = json.loads(line.strip())
            if data.get('type') in ('user', 'assistant') and data.get('message', {}).get('role') in ('user', 'assistant'):
                msg_count += 1
    except: pass
    if msg_count < 2: continue

    if name:
        named.append((rel_time, name, msg_count, session_id))
    else:
        unnamed.append((rel_time, msg_count, session_id))

r = C['r']; b = C['b']; d = C['d']; i = C['i']
cy = C['cy']; gn = C['gn']; yl = C['yl']; mg = C['mg']
dg = C['dg']; gr = C['gr']; bl = C['bl']; or_ = C['or']; wh = C['wh']

# New session
print(f'  {mg}{b}+{r}   {cy}{b}New Session{r}                                   {dg}start fresh{r}  |  __NEW__')

if named:
    print(f'  {dg}  {d}── saved ──────────────────────────────────────────────{r}  |  __HDR1__')
    for rel, name, msgs, sid in named:
        print(f'  {yl}●{r}   {b}{gn}{name:<35s}{r}  {dg}{rel:>9s}{r}  {dg}{msgs:>3d} msgs{r}  |  {sid}')

if unnamed:
    print(f'  {dg}  {d}── recent ─────────────────────────────────────────────{r}  |  __HDR2__')
    for rel, msgs, sid in unnamed:
        print(f'  {dg}○{r}   {gr}session{r}  {dg}{rel:>9s}{r}                         {dg}{msgs:>3d} msgs{r}  |  {sid}')
"

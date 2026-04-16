#!/usr/bin/env python3
"""Session stats dashboard for claude-picker"""

import json, glob, os, time
from datetime import datetime, timedelta

# Catppuccin Mocha palette
R  = '\033[0m';  B  = '\033[1m';  D  = '\033[2m';  I  = '\033[3m'
CY = '\033[38;5;117m'; GN = '\033[38;5;114m'; YL = '\033[38;5;222m'
MG = '\033[38;5;176m'; DG = '\033[38;5;242m'; GR = '\033[38;5;249m'
BL = '\033[38;5;111m'; PE = '\033[38;5;215m'; RD = '\033[38;5;203m'
WH = '\033[97m'

projects_dir = os.path.expanduser('~/.claude/projects')
sessions_dir = os.path.expanduser('~/.claude/sessions')
now = time.time()

# Load session metadata for CWD resolution
meta_cwds = {}
for mf in glob.glob(os.path.join(sessions_dir, '*.json')):
    try:
        data = json.load(open(mf))
        sid = data.get('sessionId', '')
        cwd = data.get('cwd', '')
        if sid and cwd:
            meta_cwds[sid] = cwd
    except:
        pass

# ── Scan all projects ──

total_sessions = 0
total_messages = 0
total_chars = 0
total_size = 0
project_data = {}  # project_name -> {sessions, messages, chars, tokens}
session_details = []  # (name, project, tokens)

# Activity tracking
today_count = 0
yesterday_count = 0
this_week_count = 0
older_count = 0

now_dt = datetime.now()
today_start = now_dt.replace(hour=0, minute=0, second=0, microsecond=0).timestamp()
yesterday_start = (now_dt - timedelta(days=1)).replace(hour=0, minute=0, second=0, microsecond=0).timestamp()
week_start = (now_dt - timedelta(days=now_dt.weekday())).replace(hour=0, minute=0, second=0, microsecond=0).timestamp()

for d in os.listdir(projects_dir):
    full = os.path.join(projects_dir, d)
    if not os.path.isdir(full):
        continue
    jsonl_files = glob.glob(os.path.join(full, '*.jsonl'))
    if not jsonl_files:
        continue

    # Resolve project name
    project_name = None
    for jf in jsonl_files:
        try:
            for line in open(jf):
                data = json.loads(line.strip())
                sid = data.get('sessionId', '')
                if sid and sid in meta_cwds:
                    candidate = meta_cwds[sid]
                    if os.path.isdir(candidate):
                        project_name = os.path.basename(candidate)
                        break
                break
        except:
            pass
        if project_name:
            break

    if not project_name:
        # Fallback: decode directory name
        project_name = d.split('-')[-1] if '-' in d else d

    for jf in jsonl_files:
        file_size = os.path.getsize(jf)
        mod_ts = os.path.getmtime(jf)

        # Filter: Claude CLI only
        is_claude = True
        try:
            for line in open(jf):
                data = json.loads(line.strip())
                ep = data.get('entrypoint', '')
                if ep and ep not in ('cli', 'sdk-cli'):
                    is_claude = False
                    break
                if ep in ('cli', 'sdk-cli'):
                    break
        except:
            pass
        if not is_claude:
            continue

        # Parse session
        session_name = None
        auto_name = None
        msg_count = 0
        char_count = 0
        noise = ['<local-command', '<command-name>', '<bash-', '<system-reminder>',
                 '[Request inter', '---', '<command-message>']

        try:
            for line in open(jf):
                data = json.loads(line.strip())

                if data.get('type') == 'custom-title' and data.get('customTitle'):
                    session_name = data['customTitle'][:40]

                if data.get('type') in ('user', 'assistant') and data.get('message', {}).get('role') in ('user', 'assistant'):
                    msg_count += 1
                    content = data['message'].get('content', '')
                    if isinstance(content, str):
                        char_count += len(content)
                    elif isinstance(content, list):
                        for item in content:
                            if isinstance(item, dict) and item.get('type') == 'text':
                                char_count += len(item.get('text', ''))

                    # Auto-name from first user message
                    if not auto_name and data.get('type') == 'user':
                        text = ''
                        if isinstance(content, str):
                            text = content.strip()
                        elif isinstance(content, list):
                            for item in content:
                                if isinstance(item, dict) and item.get('type') == 'text':
                                    text = item['text'].strip()
                                    break
                        if text and len(text) > 3 and not any(n in text for n in noise):
                            auto_name = text[:40].replace('\n', ' ').strip()
        except:
            pass

        if msg_count < 2:
            continue

        total_size += file_size
        total_sessions += 1
        total_messages += msg_count
        total_chars += char_count

        tokens = max(1, char_count // 4)

        # Activity bucketing
        if mod_ts >= today_start:
            today_count += 1
        elif mod_ts >= yesterday_start:
            yesterday_count += 1
        elif mod_ts >= week_start:
            this_week_count += 1
        else:
            older_count += 1

        # Project aggregation
        if project_name not in project_data:
            project_data[project_name] = {'sessions': 0, 'messages': 0, 'tokens': 0}
        project_data[project_name]['sessions'] += 1
        project_data[project_name]['messages'] += msg_count
        project_data[project_name]['tokens'] += tokens

        # Session detail for top list
        display_name = session_name or auto_name or 'unnamed'
        session_details.append((display_name, project_name, tokens))

# ── Formatting helpers ──

def format_tokens(t):
    if t >= 1_000_000:
        return f'~{t/1_000_000:.1f}M'
    elif t >= 1000:
        return f'~{t//1000}k'
    else:
        return f'~{t}'

def format_size(b):
    if b >= 1024 * 1024:
        return f'{b / (1024*1024):.0f} MB'
    elif b >= 1024:
        return f'{b // 1024} KB'
    else:
        return f'{b} B'

def bar_chart(value, max_val, max_width=20):
    if max_val == 0:
        return ''
    width = max(1, int((value / max_val) * max_width))
    return '\u2588' * width

# ── Render Dashboard ──

print()
print(f'  {MG}{B}claude-picker stats{R}')
print()

# Overview section
total_tokens = max(1, total_chars // 4)
print(f'  {DG}{D}\u2500\u2500 overview \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500{R}')
print()
print(f'    {GR}Total sessions{R}     {CY}{B}{total_sessions:,}{R}')
print(f'    {GR}Total messages{R}     {CY}{B}{total_messages:,}{R}')
print(f'    {GR}Estimated tokens{R}   {CY}{B}{format_tokens(total_tokens)}{R}')
print(f'    {GR}Disk usage{R}         {CY}{B}{format_size(total_size)}{R}')
print(f'    {GR}Projects{R}           {CY}{B}{len(project_data):,}{R}')
print()

# Projects section
if project_data:
    print(f'  {DG}{D}\u2500\u2500 projects \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500{R}')
    print()
    sorted_projects = sorted(project_data.items(), key=lambda x: x[1]['tokens'], reverse=True)
    max_proj_tokens = sorted_projects[0][1]['tokens'] if sorted_projects else 1

    for name, info in sorted_projects:
        bar = bar_chart(info['tokens'], max_proj_tokens, 16)
        s = 's' if info['sessions'] != 1 else ' '
        token_str = format_tokens(info['tokens'])
        print(f'    {GN}{B}{name:<20s}{R}  {YL}{bar:<16s}{R}  {GR}{info["sessions"]:>2d} session{s}{R}  {DG}{token_str:>8s} tokens{R}')
    print()

# Activity section
activity = [
    ('Today', today_count),
    ('Yesterday', yesterday_count),
    ('This week', this_week_count),
    ('Older', older_count),
]
max_activity = max((v for _, v in activity), default=1) or 1

print(f'  {DG}{D}\u2500\u2500 activity \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500{R}')
print()
for label, count in activity:
    bar = bar_chart(count, max_activity, 20)
    s = 's' if count != 1 else ' '
    color = CY if count > 0 else DG
    print(f'    {GR}{label:<14s}{R}  {color}{bar:<20s}{R}  {GR}{count} session{s}{R}')
print()

# Top sessions by tokens
if session_details:
    print(f'  {DG}{D}\u2500\u2500 top sessions by tokens \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500{R}')
    print()
    top = sorted(session_details, key=lambda x: x[2], reverse=True)[:5]
    for i, (name, project, tokens) in enumerate(top, 1):
        token_str = format_tokens(tokens)
        print(f'    {YL}{i}.{R} {GN}{B}{name:<28s}{R}  {DG}({project}){R}  {PE}{token_str:>8s} tokens{R}')
    print()

#!/usr/bin/env python3
"""Preview helper for claude-session-picker — world class edition"""

import json, os, sys
from datetime import datetime

session_id = sys.argv[1].strip() if len(sys.argv) > 1 else ""

R  = '\033[0m';  B  = '\033[1m';  D  = '\033[2m';  I  = '\033[3m'
CY = '\033[38;5;117m'; GN = '\033[38;5;114m'; YL = '\033[38;5;222m'
MG = '\033[38;5;176m'; DG = '\033[38;5;242m'; GR = '\033[38;5;249m'
BL = '\033[38;5;111m'; OR = '\033[38;5;215m'; WH = '\033[97m'

if not session_id or session_id in ('__NEW__',):
    print()
    print(f'  {MG}{B}New Session{R}')
    print()
    print(f'  {GR}Start a fresh Claude Code{R}')
    print(f'  {GR}conversation in this project.{R}')
    print()
    print(f'  {DG}Directory:{R}')
    print(f'  {CY}{os.path.basename(os.getcwd())}{R}')
    sys.exit(0)

if session_id in ('__HDR1__', '__HDR2__'):
    sys.exit(0)

encoded_path = os.getcwd().replace('/', '-').replace('_', '-')
project_dir = os.environ.get('PROJECT_DIR', os.path.expanduser(f'~/.claude/projects/{encoded_path}'))
session_file = os.path.join(project_dir, f'{session_id}.jsonl')

# Search mode fallback: find session across all projects
if not os.path.exists(session_file):
    import glob
    for f in glob.glob(os.path.expanduser(f'~/.claude/projects/*/{session_id}.jsonl')):
        session_file = f
        break

if not os.path.exists(session_file):
    print(f'  {DG}Session not found{R}')
    sys.exit(0)

messages = []
name = None
created = None
msg_total = 0
total_chars = 0

for line in open(session_file):
    try:
        data = json.loads(line.strip())

        if data.get('type') == 'custom-title' and data.get('customTitle'):
            name = data['customTitle']

        if not created and data.get('timestamp'):
            ts = data['timestamp']
            if isinstance(ts, str) and 'T' in ts:
                created = ts[:16].replace('T', ' ')

        msg_type = data.get('type', '')

        if msg_type in ('user', 'assistant') and data.get('message', {}).get('role') in ('user', 'assistant'):
            msg_total += 1
            content_raw = data['message'].get('content', '')
            if isinstance(content_raw, str):
                total_chars += len(content_raw)
            elif isinstance(content_raw, list):
                for ci in content_raw:
                    if isinstance(ci, dict) and ci.get('type') == 'text':
                        total_chars += len(ci.get('text', ''))

        if msg_type == 'user' and data.get('message', {}).get('role') == 'user':
            content = data['message'].get('content', '')
            text = ''
            if isinstance(content, str):
                text = content.strip()
            elif isinstance(content, list):
                for item in content:
                    if isinstance(item, dict) and item.get('type') == 'text':
                        text = item['text'].strip()
                        break
            noise = ['<local-command', '<command-name>', '<bash-', '<system-reminder>',
                     '[Request inter', '---', '<command-message>']
            if text and len(text) > 3 and not any(n in text for n in noise):
                messages.append(('you', text[:250]))

        elif msg_type == 'assistant' and data.get('message', {}).get('role') == 'assistant':
            content = data['message'].get('content', '')
            text = ''
            if isinstance(content, str):
                text = content.strip()
            elif isinstance(content, list):
                for item in content:
                    if isinstance(item, dict) and item.get('type') == 'text':
                        t = item.get('text', '').strip()
                        if t:
                            text = t[:250]
                            break
            if text and len(text) > 3:
                messages.append(('ai', text[:250]))
    except:
        pass

# Header
print()
if name:
    print(f'  {GN}{B}{name}{R}')
else:
    print(f'  {GR}{D}Unnamed session{R}')
print()

# Meta
token_est = max(1, total_chars // 4)
if token_est >= 1000:
    token_str = f'~{token_est // 1000}k'
else:
    token_str = f'~{token_est}'

if created:
    print(f'  {DG}created  {GR}{created}{R}')
print(f'  {DG}messages {GR}{msg_total}{R}')
print(f'  {DG}tokens   {GR}{token_str}{R}')
print()

# Horizontal rule between header and conversation
sep = '\u2500' * 40
print(f'  {DG}{D}{sep}{R}')
print()

# Conversation
if not messages:
    print(f'  {DG}(empty conversation){R}')
else:
    shown = messages[-8:]
    for role, text in shown:
        clean = text.replace('\n', ' ')[:140]
        if role == 'you':
            print(f'  {CY}{B}you{R}  {GR}{clean}{R}')
        else:
            print(f'  {YL}ai{R}   {WH}{clean}{R}')
        print()

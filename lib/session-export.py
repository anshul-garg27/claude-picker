#!/usr/bin/env python3
"""Export a Claude Code session to clean markdown."""

import json, os, sys, glob
from datetime import datetime

session_id = sys.argv[1].strip() if len(sys.argv) > 1 else ""
if not session_id or session_id.startswith('__'):
    sys.exit(0)

# Find session file
session_file = None
for f in glob.glob(os.path.expanduser(f'~/.claude/projects/*/{session_id}.jsonl')):
    session_file = f
    break

if not session_file:
    print(f"Session {session_id} not found", file=sys.stderr)
    sys.exit(1)

name = None
messages = []
created = None

noise = ['<local-command', '<command-name>', '<bash-', '<system-reminder>',
         '[Request inter', '---', '<command-message>', '<user-prompt']

for line in open(session_file):
    try:
        data = json.loads(line.strip())

        if data.get('type') == 'custom-title' and data.get('customTitle'):
            name = data['customTitle']

        if not created and data.get('timestamp'):
            ts = data['timestamp']
            if isinstance(ts, str) and 'T' in ts:
                created = ts[:19].replace('T', ' ')

        if data.get('type') not in ('user', 'assistant'):
            continue
        if data.get('message', {}).get('role') not in ('user', 'assistant'):
            continue

        role = data['type']
        content = data['message'].get('content', '')
        text = ''
        if isinstance(content, str):
            text = content.strip()
        elif isinstance(content, list):
            for item in content:
                if isinstance(item, dict) and item.get('type') == 'text':
                    text = item.get('text', '').strip()
                    break

        if not text or len(text) < 3:
            continue
        if any(n in text for n in noise):
            continue

        messages.append((role, text))
    except:
        pass

# Build markdown
title = name or f'Session {session_id[:8]}'
md = []
md.append(f'# {title}')
md.append('')
if created:
    md.append(f'**Created:** {created}')
md.append(f'**Messages:** {len(messages)}')
md.append(f'**Session ID:** `{session_id}`')
md.append('')
md.append('---')
md.append('')

for role, text in messages:
    if role == 'user':
        md.append(f'## You')
    else:
        md.append(f'## Claude')
    md.append('')
    md.append(text)
    md.append('')

# Write to file
export_dir = os.path.expanduser('~/Desktop/claude-exports')
os.makedirs(export_dir, exist_ok=True)

safe_name = ''.join(c if c.isalnum() or c in '-_ ' else '' for c in title).strip().replace(' ', '-')
filename = f'{safe_name}.md'
filepath = os.path.join(export_dir, filename)

with open(filepath, 'w') as f:
    f.write('\n'.join(md))

print(filepath)

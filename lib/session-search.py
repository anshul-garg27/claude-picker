#!/usr/bin/env python3
"""Full-text search across all Claude Code sessions.

Searches user and assistant messages in all JSONL session files.
Outputs results formatted for fzf with session context.
"""

import json, glob, os, sys, re

R  = '\033[0m';  B  = '\033[1m';  D  = '\033[2m';  I  = '\033[3m'
CY = '\033[38;5;117m'; GN = '\033[38;5;114m'; YL = '\033[38;5;222m'
MG = '\033[38;5;176m'; DG = '\033[38;5;242m'; GR = '\033[38;5;249m'
RD = '\033[38;5;203m'

projects_dir = os.path.expanduser('~/.claude/projects')
sessions_dir = os.path.expanduser('~/.claude/sessions')

noise = ['<local-command', '<command-name>', '<bash-', '<system-reminder>',
         '[Request inter', '---', '<command-message>', '<user-prompt']

# Load metadata for cwd resolution
meta_by_sid = {}
for mf in glob.glob(os.path.join(sessions_dir, '*.json')):
    try:
        data = json.load(open(mf))
        sid = data.get('sessionId', '')
        if sid:
            meta_by_sid[sid] = data
    except:
        pass

results = []

for proj_dir in sorted(os.listdir(projects_dir)):
    full_proj = os.path.join(projects_dir, proj_dir)
    if not os.path.isdir(full_proj):
        continue

    for jf in glob.glob(os.path.join(full_proj, '*.jsonl')):
        session_id = os.path.basename(jf).replace('.jsonl', '')
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

        # Get session name
        session_name = None
        try:
            for line in open(jf):
                data = json.loads(line.strip())
                if data.get('type') == 'custom-title' and data.get('customTitle'):
                    session_name = data['customTitle'][:30]
                    break
        except:
            pass
        if not session_name:
            meta = meta_by_sid.get(session_id, {})
            session_name = meta.get('name', '')[:30] if meta.get('name') else None

        # Resolve project name
        proj_name = None
        for sid_key, meta in meta_by_sid.items():
            cwd = meta.get('cwd', '')
            if cwd and cwd.replace('/', '-').replace('_', '-') == proj_dir:
                proj_name = os.path.basename(cwd)
                break
        # Fallback: try cwd from jsonl
        if not proj_name:
            try:
                for line in open(jf):
                    data = json.loads(line.strip())
                    if 'cwd' in data and data['cwd']:
                        proj_name = os.path.basename(data['cwd'])
                        break
            except:
                pass
        if not proj_name:
            proj_name = proj_dir.split('-')[-1][:20]

        # Extract searchable messages
        try:
            for line in open(jf):
                data = json.loads(line.strip())
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

                if not text or len(text) < 5:
                    continue
                if any(n in text for n in noise):
                    continue

                # Clean and truncate
                clean = text.replace('\n', ' ')[:200]

                role_label = f'{CY}you{R}' if role == 'user' else f'{YL}ai{R}'
                name_label = f'{GN}{session_name}{R}' if session_name else f'{DG}session{R}'
                proj_label = f'{MG}{proj_name}{R}'

                # Format: visible content [TAB] session_id (TAB is fzf delimiter)
                visible = f'  {proj_label}  {DG}\u2502{R}  {name_label}  {DG}\u2502{R}  {role_label}  {DG}\u2502{R}  {GR}{clean}{R}'
                print(visible + '\t' + session_id)
        except:
            pass

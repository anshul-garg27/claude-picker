#!/usr/bin/env python3
"""Preview renderer using Rich for beautiful formatted output."""

import json, os, sys, glob

session_id = sys.argv[1].strip() if len(sys.argv) > 1 else ""

# Try Rich, fall back to plain ANSI
try:
    from rich.console import Console
    from rich.panel import Panel
    from rich.text import Text
    from rich.table import Table
    from rich import box
    HAS_RICH = True
except ImportError:
    HAS_RICH = False

# Plain ANSI fallback colors
R  = '\033[0m';  B  = '\033[1m';  D  = '\033[2m';  I  = '\033[3m'
CY = '\033[38;5;117m'; GN = '\033[38;5;114m'; YL = '\033[38;5;222m'
MG = '\033[38;5;176m'; DG = '\033[38;5;242m'; GR = '\033[38;5;249m'
WH = '\033[97m'

if not session_id or session_id in ('__NEW__',):
    if HAS_RICH:
        console = Console(force_terminal=True, width=60)
        console.print()
        console.print("  [bold #cba6f7]+ New Session[/]")
        console.print()
        console.print("  [#a6adc8]Start a fresh Claude Code[/]")
        console.print("  [#a6adc8]conversation in this project.[/]")
        console.print()
        console.print(f"  [#6c7086]Directory:[/] [#89b4fa]{os.path.basename(os.getcwd())}[/]")
    else:
        print(f'\n  {MG}{B}+ New Session{R}\n')
        print(f'  {GR}Start a fresh Claude Code conversation.{R}\n')
        print(f'  {DG}Directory:{R} {CY}{os.path.basename(os.getcwd())}{R}')
    sys.exit(0)

if session_id in ('__HDR0__', '__HDR1__', '__HDR2__', '__SEP__', '__INFO__'):
    sys.exit(0)

# Find session file
encoded_path = os.getcwd().replace('/', '-').replace('_', '-')
project_dir = os.environ.get('PROJECT_DIR', os.path.expanduser(f'~/.claude/projects/{encoded_path}'))
session_file = os.path.join(project_dir, f'{session_id}.jsonl')

if not os.path.exists(session_file):
    for f in glob.glob(os.path.expanduser(f'~/.claude/projects/*/{session_id}.jsonl')):
        session_file = f
        break

if not os.path.exists(session_file):
    print(f'  {DG}Session not found{R}')
    sys.exit(0)

# Parse session
messages = []
name = None
created = None
msg_total = 0
total_chars = 0

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
            if text and len(text) > 3 and not any(n in text for n in noise):
                messages.append(('you', text[:300]))

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
                            text = t[:300]
                            break
            if text and len(text) > 3:
                messages.append(('ai', text[:300]))
    except:
        pass

# Token estimate
token_est = max(1, total_chars // 4)
token_str = f'~{token_est // 1000}k' if token_est >= 1000 else f'~{token_est}'

# Render with Rich
if HAS_RICH:
    console = Console(force_terminal=True, width=60)

    # Header
    title = name or "Unnamed session"
    title_style = "bold #a6e3a1" if name else "dim #a6adc8"

    meta_table = Table(show_header=False, box=None, padding=(0, 1), show_edge=False)
    meta_table.add_column(style="#6c7086", width=10)
    meta_table.add_column(style="#a6adc8")
    if created:
        meta_table.add_row("created", created)
    meta_table.add_row("messages", str(msg_total))
    meta_table.add_row("tokens", token_str)

    console.print()
    console.print(f"  [{title_style}]{title}[/]")
    console.print()
    console.print(meta_table)
    console.print()
    console.print(f"  [#45475a]{'─' * 40}[/]")
    console.print()

    if not messages:
        console.print("  [#6c7086](empty conversation)[/]")
    else:
        shown = messages[-8:]
        for role, text in shown:
            clean = text.replace('\n', ' ')[:140]
            if role == 'you':
                console.print(f"  [bold #89b4fa]you[/]  [#a6adc8]{clean}[/]")
            else:
                console.print(f"  [#f9e2af]ai[/]   [#cdd6f4]{clean}[/]")
            console.print()
else:
    # Fallback to plain ANSI
    print()
    if name:
        print(f'  {GN}{B}{name}{R}')
    else:
        print(f'  {GR}{D}Unnamed session{R}')
    print()
    if created:
        print(f'  {DG}created  {GR}{created}{R}')
    print(f'  {DG}messages {GR}{msg_total}{R}')
    print(f'  {DG}tokens   {GR}{token_str}{R}')
    print()
    print(f'  {DG}{"─" * 40}{R}')
    print()
    if not messages:
        print(f'  {DG}(empty conversation){R}')
    else:
        for role, text in messages[-8:]:
            clean = text.replace('\n', ' ')[:140]
            if role == 'you':
                print(f'  {CY}{B}you{R}  {GR}{clean}{R}')
            else:
                print(f'  {YL}ai{R}   {WH}{clean}{R}')
            print()

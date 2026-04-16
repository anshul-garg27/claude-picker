#!/usr/bin/env python3
"""Fork tree visualization for Claude Code sessions.

Scans all sessions across all projects, detects fork relationships
(shared message UUIDs or parentSessionId references), and displays
an ASCII tree showing parent-child session relationships.

If no forks exist, shows a flat list grouped by project.
"""

import json, glob, os, sys, time
from datetime import datetime
from collections import defaultdict

# Catppuccin Mocha palette
R  = '\033[0m';  B  = '\033[1m';  D  = '\033[2m';  I  = '\033[3m'
CY = '\033[38;5;117m'; GN = '\033[38;5;114m'; YL = '\033[38;5;222m'
MG = '\033[38;5;176m'; DG = '\033[38;5;242m'; GR = '\033[38;5;249m'
BL = '\033[38;5;111m'; OR = '\033[38;5;215m'; RD = '\033[38;5;203m'
WH = '\033[97m'

projects_dir = os.path.expanduser('~/.claude/projects')
sessions_dir = os.path.expanduser('~/.claude/sessions')
now = time.time()

noise = ['<local-command', '<command-name>', '<bash-', '<system-reminder>',
         '[Request inter', '---', '<command-message>', '<user-prompt']


def relative_time(ts):
    diff = now - ts
    if diff < 60: return 'just now'
    elif diff < 3600: return f'{int(diff/60)}m ago'
    elif diff < 86400: return f'{int(diff/3600)}h ago'
    elif diff < 604800: return f'{int(diff/86400)}d ago'
    else: return datetime.fromtimestamp(ts).strftime('%b %d')


# ── Load session metadata ──
meta_by_sid = {}
for mf in glob.glob(os.path.join(sessions_dir, '*.json')):
    try:
        data = json.load(open(mf))
        sid = data.get('sessionId', '')
        if sid:
            meta_by_sid[sid] = data
    except:
        pass

# ── Scan all sessions ──
# session_info[session_id] = {name, project, mod_ts, msg_count, first_user_msg, uuids, parent_session}
session_info = {}
# Map from message uuid -> session_id (for fork detection)
uuid_to_session = {}

for proj_dir_name in sorted(os.listdir(projects_dir)):
    full_proj = os.path.join(projects_dir, proj_dir_name)
    if not os.path.isdir(full_proj):
        continue

    # Resolve project name
    proj_name = None
    for sid_key, meta in meta_by_sid.items():
        cwd = meta.get('cwd', '')
        if cwd and cwd.replace('/', '-').replace('_', '-') == proj_dir_name:
            proj_name = os.path.basename(cwd)
            break
    if not proj_name:
        proj_name = proj_dir_name.split('-')[-1][:20]

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

        name = None
        auto_name = None
        msg_count = 0
        uuids = set()
        parent_uuids = set()
        parent_session = None

        try:
            for line in open(jf):
                data = json.loads(line.strip())

                if data.get('type') == 'custom-title' and data.get('customTitle'):
                    name = data['customTitle'][:35]

                # Check for fork/parent session reference
                if data.get('type') == 'fork' and data.get('parentSessionId'):
                    parent_session = data['parentSessionId']

                # Collect message UUIDs for fork detection
                if data.get('uuid'):
                    uuids.add(data['uuid'])
                if data.get('parentUuid'):
                    parent_uuids.add(data['parentUuid'])

                # Count messages
                if data.get('type') in ('user', 'assistant') and \
                   data.get('message', {}).get('role') in ('user', 'assistant'):
                    msg_count += 1

                    # Auto-name: first meaningful user message
                    if not auto_name and data.get('type') == 'user':
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
                            auto_name = text[:50].replace('\n', ' ').strip()
        except:
            pass

        if not name:
            meta = meta_by_sid.get(session_id, {})
            if meta.get('name'):
                name = meta['name'][:35]

        if msg_count < 2:
            continue

        display_name = name or (auto_name[:35] if auto_name else 'session')

        session_info[session_id] = {
            'name': display_name,
            'named': bool(name),
            'project': proj_name,
            'proj_dir': proj_dir_name,
            'mod_ts': mod_ts,
            'msg_count': msg_count,
            'uuids': uuids,
            'parent_uuids': parent_uuids,
            'parent_session': parent_session,
        }

        for u in uuids:
            uuid_to_session[u] = session_id

# ── Detect fork relationships ──
# A session B is a fork of session A if:
#   1. B has an explicit parentSessionId pointing to A, OR
#   2. B references parentUuids that belong to A's uuid set
#      (shared message history = fork)

children = defaultdict(list)  # parent_sid -> [child_sid, ...]
has_parent = set()

for sid, info in session_info.items():
    # Method 1: explicit fork reference
    if info['parent_session'] and info['parent_session'] in session_info:
        children[info['parent_session']].append(sid)
        has_parent.add(sid)
        continue

    # Method 2: shared UUID detection
    # If a session's parentUuids reference UUIDs owned by another session,
    # and that session has no reciprocal reference, it's a fork
    parent_candidates = defaultdict(int)
    for pu in info['parent_uuids']:
        if pu in uuid_to_session:
            other_sid = uuid_to_session[pu]
            if other_sid != sid:
                parent_candidates[other_sid] += 1

    # The session with the most shared parentUuids is likely the parent
    # But only if the overlap is significant (more than just noise)
    if parent_candidates:
        best_parent = max(parent_candidates, key=parent_candidates.get)
        shared_count = parent_candidates[best_parent]
        # Only consider it a fork if there are meaningful shared references
        if shared_count >= 3:
            children[best_parent].append(sid)
            has_parent.add(sid)

has_forks = bool(children)

# ── Print header ──
print()
total = len(session_info)
fork_count = sum(len(c) for c in children.values())
if has_forks:
    print(f'  {MG}{B}session tree{R}  {DG}│  {total} sessions  {fork_count} fork{"s" if fork_count != 1 else ""}{R}')
else:
    print(f'  {MG}{B}session tree{R}  {DG}│  {total} sessions  no forks detected{R}')
print()


def print_tree_node(sid, prefix='', is_last=True, depth=0):
    """Recursively print a session and its children as an ASCII tree."""
    info = session_info[sid]
    connector = '└── ' if is_last else '├── '
    if depth == 0:
        connector = ''
        line_prefix = '  '
    else:
        line_prefix = f'  {prefix}{connector}'

    name_color = GN if info['named'] else GR
    icon = '●' if info['named'] else '○'
    rel = relative_time(info['mod_ts'])

    print(f'{line_prefix}{YL}{icon}{R} {name_color}{B}{info["name"]:<30s}{R}  '
          f'{DG}{rel:>9s}{R}  {DG}{info["msg_count"]:>3d} msgs{R}')

    child_sids = sorted(children.get(sid, []),
                        key=lambda s: session_info[s]['mod_ts'], reverse=True)
    for i, child_sid in enumerate(child_sids):
        is_child_last = (i == len(child_sids) - 1)
        new_prefix = prefix + ('    ' if is_last else '│   ')
        if depth == 0:
            new_prefix = '    '
        print_tree_node(child_sid, new_prefix, is_child_last, depth + 1)


if has_forks:
    # ── Tree view: show sessions with fork relationships ──
    # Group by project
    projects = defaultdict(list)
    for sid, info in session_info.items():
        projects[info['project']].append(sid)

    for proj_name in sorted(projects.keys()):
        sids = projects[proj_name]
        # Find root sessions (no parent)
        roots = [s for s in sids if s not in has_parent]
        # Also include sessions that are parents
        all_tree_sids = set()
        for s in sids:
            if s in children or s in has_parent:
                all_tree_sids.add(s)
        # Show standalone sessions too
        standalone = [s for s in sids if s not in all_tree_sids]

        print(f'  {CY}{B}{proj_name}{R}')
        sep = '─' * 55
        print(f'  {DG}{D}{sep}{R}')

        # Print tree roots first
        tree_roots = sorted([s for s in roots if s in all_tree_sids or s in children],
                            key=lambda s: session_info[s]['mod_ts'], reverse=True)
        for sid in tree_roots:
            print_tree_node(sid)

        # Then standalone sessions
        standalone_sorted = sorted(standalone,
                                   key=lambda s: session_info[s]['mod_ts'], reverse=True)
        for sid in standalone_sorted:
            info = session_info[sid]
            name_color = GN if info['named'] else GR
            icon = '●' if info['named'] else '○'
            rel = relative_time(info['mod_ts'])
            print(f'  {DG}{icon}{R} {name_color}{info["name"]:<30s}{R}  '
                  f'{DG}{rel:>9s}{R}  {DG}{info["msg_count"]:>3d} msgs{R}')
        print()

else:
    # ── Flat view: group by project ──
    projects = defaultdict(list)
    for sid, info in session_info.items():
        projects[info['project']].append(sid)

    for proj_name in sorted(projects.keys()):
        sids = sorted(projects[proj_name],
                      key=lambda s: session_info[s]['mod_ts'], reverse=True)

        print(f'  {CY}{B}{proj_name}{R}  {DG}({len(sids)} session{"s" if len(sids) != 1 else ""}){R}')
        sep = '─' * 55
        print(f'  {DG}{D}{sep}{R}')

        for sid in sids:
            info = session_info[sid]
            name_color = GN if info['named'] else GR
            icon = '●' if info['named'] else '○'
            rel = relative_time(info['mod_ts'])
            print(f'  {DG}{icon}{R} {name_color}{info["name"]:<30s}{R}  '
                  f'{DG}{rel:>9s}{R}  {DG}{info["msg_count"]:>3d} msgs{R}')
        print()

print(f'  {DG}{D}tip: use --fork-session <id> in claude to create forks{R}')
print()

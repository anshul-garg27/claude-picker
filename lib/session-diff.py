#!/usr/bin/env python3
"""Compare two Claude Code sessions side by side.

Takes two session IDs as arguments.
Extracts messages, identifies topics, and shows a visual comparison
highlighting common and unique topics per session.
"""

import json, glob, os, sys, re
from collections import Counter

# Catppuccin Mocha palette
R  = '\033[0m';  B  = '\033[1m';  D  = '\033[2m';  I  = '\033[3m'
CY = '\033[38;5;117m'; GN = '\033[38;5;114m'; YL = '\033[38;5;222m'
MG = '\033[38;5;176m'; DG = '\033[38;5;242m'; GR = '\033[38;5;249m'
BL = '\033[38;5;111m'; OR = '\033[38;5;215m'; WH = '\033[97m'
RD = '\033[38;5;203m'

noise = ['<local-command', '<command-name>', '<bash-', '<system-reminder>',
         '[Request inter', '---', '<command-message>', '<user-prompt']

# Common stop words to filter from topic extraction
STOP_WORDS = {
    'the', 'a', 'an', 'is', 'are', 'was', 'were', 'be', 'been', 'being',
    'have', 'has', 'had', 'do', 'does', 'did', 'will', 'would', 'could',
    'should', 'may', 'might', 'shall', 'can', 'need', 'dare', 'ought',
    'used', 'to', 'of', 'in', 'for', 'on', 'with', 'at', 'by', 'from',
    'as', 'into', 'through', 'during', 'before', 'after', 'above', 'below',
    'between', 'out', 'off', 'over', 'under', 'again', 'further', 'then',
    'once', 'here', 'there', 'when', 'where', 'why', 'how', 'all', 'both',
    'each', 'few', 'more', 'most', 'other', 'some', 'such', 'no', 'nor',
    'not', 'only', 'own', 'same', 'so', 'than', 'too', 'very', 'just',
    'because', 'but', 'and', 'or', 'if', 'while', 'about', 'up', 'what',
    'which', 'who', 'whom', 'this', 'that', 'these', 'those', 'am', 'it',
    'its', 'my', 'your', 'his', 'her', 'our', 'their', 'me', 'him', 'us',
    'them', 'i', 'you', 'he', 'she', 'we', 'they', 'also', 'like', 'make',
    'get', 'got', 'let', 'want', 'know', 'think', 'see', 'look', 'use',
    'file', 'code', 'please', 'sure', 'okay', 'yes', 'right', 'well',
    'don', 'doesn', 'didn', 'won', 'wouldn', 'couldn', 'shouldn', 'hasn',
    'haven', 'wasn', 'weren', 'isn', 'aren', 'now', 'going', 'using',
    've', 'll', 're', 'new', 'one', 'two', 'first', 'way', 'work',
}


def find_session_file(session_id):
    """Find a session JSONL file across all projects."""
    for f in glob.glob(os.path.expanduser(f'~/.claude/projects/*/{session_id}.jsonl')):
        return f
    return None


def extract_session_data(session_file):
    """Extract name, messages, and text content from a session file."""
    name = None
    messages = []
    all_text = []
    created = None

    for line in open(session_file):
        try:
            data = json.loads(line.strip())

            if data.get('type') == 'custom-title' and data.get('customTitle'):
                name = data['customTitle']

            if not created and data.get('timestamp'):
                ts = data['timestamp']
                if isinstance(ts, str) and 'T' in ts:
                    created = ts[:16].replace('T', ' ')

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
            all_text.append(text.lower())
        except:
            pass

    return {
        'name': name or f'Session {os.path.basename(session_file)[:8]}',
        'created': created,
        'messages': messages,
        'all_text': all_text,
        'msg_count': len(messages),
    }


def extract_topics(texts, top_n=15):
    """Extract meaningful topic words/phrases from text content."""
    word_freq = Counter()

    for text in texts:
        # Extract words (3+ chars, alphanumeric)
        words = re.findall(r'\b[a-z][a-z0-9_]{2,}\b', text.lower())
        for w in words:
            if w not in STOP_WORDS and len(w) > 2:
                word_freq[w] += 1

    # Also extract bigrams for compound topics
    bigram_freq = Counter()
    for text in texts:
        words = [w for w in re.findall(r'\b[a-z][a-z0-9_]{2,}\b', text.lower())
                 if w not in STOP_WORDS]
        for i in range(len(words) - 1):
            bigram = f'{words[i]} {words[i+1]}'
            bigram_freq[bigram] += 1

    # Merge: prefer bigrams that appear 2+ times
    topics = {}
    for bigram, count in bigram_freq.most_common(top_n * 2):
        if count >= 2:
            topics[bigram] = count * 2  # Weight bigrams higher

    for word, count in word_freq.most_common(top_n * 3):
        if count >= 2 and word not in topics:
            # Skip if word is already part of a bigram topic
            in_bigram = any(word in bg for bg in topics)
            if not in_bigram:
                topics[word] = count

    # Return top N by frequency
    sorted_topics = sorted(topics.items(), key=lambda x: x[1], reverse=True)
    return [t[0] for t in sorted_topics[:top_n]]


def print_separator(width=60):
    print(f'  {DG}{D}{"─" * width}{R}')


# ── Main ──
if len(sys.argv) < 3:
    print(f"Usage: session-diff.py <session_id_a> <session_id_b>", file=sys.stderr)
    sys.exit(1)

sid_a = sys.argv[1].strip()
sid_b = sys.argv[2].strip()

file_a = find_session_file(sid_a)
file_b = find_session_file(sid_b)

if not file_a:
    print(f"  {RD}Session A not found:{R} {sid_a}", file=sys.stderr)
    sys.exit(1)
if not file_b:
    print(f"  {RD}Session B not found:{R} {sid_b}", file=sys.stderr)
    sys.exit(1)

data_a = extract_session_data(file_a)
data_b = extract_session_data(file_b)

topics_a = set(extract_topics(data_a['all_text']))
topics_b = set(extract_topics(data_b['all_text']))

common_topics = topics_a & topics_b
unique_a = topics_a - topics_b
unique_b = topics_b - topics_a

# ── Header ──
print()
print(f'  {MG}{B}session diff{R}')
print()

# ── Session info side by side ──
col_w = 35
print(f'  {GN}{B}{"Session A":<{col_w}s}{R}  {DG}│{R}  {YL}{B}{"Session B":<{col_w}s}{R}')
print_separator()

name_a = data_a['name'][:col_w]
name_b = data_b['name'][:col_w]
print(f'  {GN}{name_a:<{col_w}s}{R}  {DG}│{R}  {YL}{name_b:<{col_w}s}{R}')

created_a = (data_a['created'] or 'unknown')[:col_w]
created_b = (data_b['created'] or 'unknown')[:col_w]
print(f'  {DG}{created_a:<{col_w}s}{R}  {DG}│{R}  {DG}{created_b:<{col_w}s}{R}')

msgs_a = f'{data_a["msg_count"]} messages'
msgs_b = f'{data_b["msg_count"]} messages'
print(f'  {DG}{msgs_a:<{col_w}s}{R}  {DG}│{R}  {DG}{msgs_b:<{col_w}s}{R}')

print()
print_separator()

# ── Common topics ──
print()
print(f'  {CY}{B}common topics{R}  {DG}({len(common_topics)}){R}')
if common_topics:
    line = '  '
    for topic in sorted(common_topics):
        tag = f'{CY}{topic}{R}  '
        line += tag
        if len(line) > 120:
            print(line)
            line = '  '
    if line.strip():
        print(line)
else:
    print(f'  {DG}(no common topics){R}')

print()
print_separator()

# ── Unique to Session A ──
print()
print(f'  {GN}{B}unique to Session A{R}  {DG}({len(unique_a)}){R}')
if unique_a:
    line = '  '
    for topic in sorted(unique_a):
        tag = f'{GN}{topic}{R}  '
        line += tag
        if len(line) > 120:
            print(line)
            line = '  '
    if line.strip():
        print(line)
else:
    print(f'  {DG}(no unique topics){R}')

print()
print_separator()

# ── Unique to Session B ──
print()
print(f'  {YL}{B}unique to Session B{R}  {DG}({len(unique_b)}){R}')
if unique_b:
    line = '  '
    for topic in sorted(unique_b):
        tag = f'{YL}{topic}{R}  '
        line += tag
        if len(line) > 120:
            print(line)
            line = '  '
    if line.strip():
        print(line)
else:
    print(f'  {DG}(no unique topics){R}')

print()
print_separator()

# ── Conversation preview comparison ──
print()
print(f'  {MG}{B}conversation preview{R}')
print()

max_preview = 5

print(f'  {GN}{B}Session A{R}  {DG}─ {data_a["name"]}{R}')
print()
shown_a = data_a['messages'][:max_preview]
for role, text in shown_a:
    clean = text.replace('\n', ' ')[:100]
    if role == 'user':
        print(f'    {CY}you{R}  {GR}{clean}{R}')
    else:
        print(f'    {YL}ai{R}   {WH}{clean}{R}')
if len(data_a['messages']) > max_preview:
    remaining = len(data_a['messages']) - max_preview
    print(f'    {DG}... +{remaining} more messages{R}')

print()
print_separator()
print()

print(f'  {YL}{B}Session B{R}  {DG}─ {data_b["name"]}{R}')
print()
shown_b = data_b['messages'][:max_preview]
for role, text in shown_b:
    clean = text.replace('\n', ' ')[:100]
    if role == 'user':
        print(f'    {CY}you{R}  {GR}{clean}{R}')
    else:
        print(f'    {YL}ai{R}   {WH}{clean}{R}')
if len(data_b['messages']) > max_preview:
    remaining = len(data_b['messages']) - max_preview
    print(f'    {DG}... +{remaining} more messages{R}')

print()

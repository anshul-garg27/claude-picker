#!/bin/bash
# claude-picker demo mode — hardcoded data for GIF recording
# This produces the exact same visual output without needing real sessions

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Colors
R='\033[0m'; B='\033[1m'; D='\033[2m'
CY='\033[38;5;117m'; GN='\033[38;5;114m'; YL='\033[38;5;222m'
MG='\033[38;5;176m'; DG='\033[38;5;242m'; GR='\033[38;5;249m'

# Step 1: Project picker
dir_list=$(echo -e "\
  ${CY}${B}architex                      ${R}  ${DG} just now${R}  ${GN}${D}████████████${R} ${DG}14 sessions${R}  |  /projects/architex
  ${CY}${B}my-saas-app                   ${R}  ${DG}   3h ago${R}  ${GN}${D}███████     ${R} ${DG}7 sessions${R}  |  /projects/my-saas-app
  ${CY}${B}dotfiles                      ${R}  ${DG}   1d ago${R}  ${GN}${D}███         ${R} ${DG}3 sessions${R}  |  /projects/dotfiles
  ${CY}${B}api-gateway                   ${R}  ${DG}  Apr 12${R}  ${GN}${D}█████       ${R} ${DG}5 sessions${R}  |  /projects/api-gateway")

selected_dir=$(echo -e "$dir_list" | fzf \
  --header=$'  \033[38;5;176m\033[1mclaude-picker\033[0m\n' \
  --header-first \
  --delimiter="|" \
  --with-nth=1 \
  --layout=reverse \
  --height=50% \
  --border=rounded \
  --margin=1,3 \
  --padding=1,1 \
  --prompt="  project > " \
  --pointer=" ▸" \
  --color="fg:249,fg+:117,bg+:-1,hl:222,hl+:222,pointer:176,prompt:117,header:176,border:238,gutter:-1" \
  --no-separator \
  --preview-window=hidden \
  --ansi)

[ -z "$selected_dir" ] && exit 0

# Step 2: Session picker
session_list=$(echo -e "\
  ${MG}${B}+${R}   ${CY}${B}New Session${R}                                   ${DG}start fresh${R}  |  __NEW__
  ${DG}  ${D}── saved ──────────────────────────────────────────────${R}  |  __HDR1__
  ${YL}●${R}   ${B}${GN}auth-refactor                      ${R}  ${DG}   5m ago${R}  ${DG} 45 msgs${R}  |  abc123
  ${YL}●${R}   ${B}${GN}fix-race-condition                 ${R}  ${DG}   2h ago${R}  ${DG} 28 msgs${R}  |  def456
  ${YL}●${R}   ${B}${GN}drizzle-migration                  ${R}  ${DG}   1d ago${R}  ${DG} 67 msgs${R}  |  ghi789
  ${YL}●${R}   ${B}${GN}mcp-postgres-setup                 ${R}  ${DG}   3d ago${R}  ${DG} 12 msgs${R}  |  jkl012
  ${DG}  ${D}── recent ─────────────────────────────────────────────${R}  |  __HDR2__
  ${DG}○${R}   ${GR}session${R}  ${DG}   4h ago${R}                         ${DG} 31 msgs${R}  |  mno345
  ${DG}○${R}   ${GR}session${R}  ${DG}   1d ago${R}                         ${DG}  8 msgs${R}  |  pqr678
  ${DG}○${R}   ${GR}session${R}  ${DG}   2d ago${R}                         ${DG} 19 msgs${R}  |  stu901")

# Create fake preview script
PREVIEW_SCRIPT=$(mktemp)
cat > "$PREVIEW_SCRIPT" << 'PYEOF'
#!/usr/bin/env python3
import sys
sid = sys.argv[1].strip() if len(sys.argv) > 1 else ""

R  = '\033[0m';  B  = '\033[1m';  D  = '\033[2m'
CY = '\033[38;5;117m'; GN = '\033[38;5;114m'; YL = '\033[38;5;222m'
MG = '\033[38;5;176m'; DG = '\033[38;5;242m'; GR = '\033[38;5;249m'
WH = '\033[97m'

previews = {
    "__NEW__": f"""
  {MG}{B}New Session{R}

  {GR}Start a fresh Claude Code{R}
  {GR}conversation in this project.{R}
""",
    "abc123": f"""
  {GN}{B}auth-refactor{R}

  {DG}created  {GR}2026-04-16 14:30{R}
  {DG}messages {GR}45{R}

  {DG}{D}{'─' * 36}{R}

  {CY}{B}you{R}  {GR}the auth middleware is storing session tokens in a way that doesn't meet compliance. can you refactor it?{R}

  {YL}ai{R}   {WH}I'll restructure the session token storage to use encrypted HttpOnly cookies with proper rotation...{R}

  {CY}{B}you{R}  {GR}also need to handle the refresh token flow. currently it's broken when the access token expires{R}

  {YL}ai{R}   {WH}Found the issue. The refresh endpoint wasn't checking token expiry correctly. Here's the fix...{R}

  {CY}{B}you{R}  {GR}perfect. now let's add rate limiting to the auth endpoints{R}

  {YL}ai{R}   {WH}I'll add a sliding window rate limiter using Redis. 5 attempts per minute per IP for login...{R}
""",
    "def456": f"""
  {GN}{B}fix-race-condition{R}

  {DG}created  {GR}2026-04-16 12:15{R}
  {DG}messages {GR}28{R}

  {DG}{D}{'─' * 36}{R}

  {CY}{B}you{R}  {GR}there's a race condition in the order processing. two requests can claim the same inventory{R}

  {YL}ai{R}   {WH}Classic double-spend problem. Let me look at the order service... Found it. The SELECT and UPDATE aren't atomic.{R}

  {CY}{B}you{R}  {GR}we're using postgres. can we use advisory locks?{R}

  {YL}ai{R}   {WH}Yes, pg_advisory_xact_lock is perfect here. It'll hold the lock for the transaction duration...{R}
""",
    "ghi789": f"""
  {GN}{B}drizzle-migration{R}

  {DG}created  {GR}2026-04-15 09:45{R}
  {DG}messages {GR}67{R}

  {DG}{D}{'─' * 36}{R}

  {CY}{B}you{R}  {GR}need to add a new table for user preferences with a jsonb column{R}

  {YL}ai{R}   {WH}I'll create the migration. Using Drizzle's pgTable with a jsonb column for flexible preferences...{R}

  {CY}{B}you{R}  {GR}also add an index on the user_id foreign key{R}

  {YL}ai{R}   {WH}Added a btree index on user_id. Here's the complete migration file...{R}
""",
}

default = f"""
  {GR}{D}Unnamed session{R}

  {DG}{D}{'─' * 36}{R}

  {CY}{B}you{R}  {GR}can you help me debug this failing test?{R}

  {YL}ai{R}   {WH}Let me look at the test file and the implementation...{R}
"""

print(previews.get(sid, default))
PYEOF
chmod +x "$PREVIEW_SCRIPT"

selected=$(echo -e "$session_list" | fzf \
  --header=$'  \033[38;5;176m\033[1marchitex\033[0m  \033[38;5;242m│  enter open  │  ctrl-d delete  │  ctrl-c back\033[0m\n' \
  --header-first \
  --delimiter="|" \
  --with-nth=1 \
  --layout=reverse \
  --height=85% \
  --border=rounded \
  --margin=1,3 \
  --padding=1,1 \
  --prompt="  session > " \
  --pointer=" ▸" \
  --color="fg:249,fg+:114,bg+:-1,hl:222,hl+:222,pointer:176,prompt:117,header:176,border:238,gutter:-1" \
  --no-separator \
  --preview="python3 '$PREVIEW_SCRIPT' \$(echo {} | awk -F'|' '{print \$NF}' | xargs)" \
  --preview-window=right,45%,wrap,border-left \
  --ansi)

rm -f "$PREVIEW_SCRIPT"

if [ -n "$selected" ]; then
  sid=$(echo "$selected" | awk -F'|' '{print $NF}' | xargs)
  echo ""
  echo -e "  ${GN}${B}Resuming session: ${sid}${R}"
  echo -e "  ${DG}claude --resume ${sid}${R}"
  echo ""
fi

#!/bin/bash
# Hardcoded demo of claude-picker --search flow, for GIF recording.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

R='\033[0m'; B='\033[1m'; D='\033[2m'
CY='\033[38;5;117m'; GN='\033[38;5;114m'; YL='\033[38;5;222m'
MG='\033[38;5;176m'; DG='\033[38;5;242m'; GR='\033[38;5;249m'
WH='\033[97m'

results=$(echo -e "\
  ${MG}▸${R} ${B}${GN}auth-refactor${R}              ${DG}architex${R}          ${WH}refactor the auth middleware to use encrypted HttpOnly cookies with proper rotation${R}  |  abc123|architex
  ${MG}▸${R} ${B}${GN}auth-refactor${R}              ${DG}architex${R}          ${WH}also need to handle the refresh token flow when the access token expires${R}  |  abc123|architex
  ${MG}▸${R} ${B}${GN}fix-oauth-flow${R}             ${DG}api-gateway${R}       ${WH}the oauth callback is failing because of a CORS issue on the auth endpoint${R}  |  xyz789|api-gateway
  ${MG}▸${R} ${B}${GN}middleware-refactor${R}        ${DG}api-gateway${R}       ${WH}move all the auth middleware into a shared package so other services can use it${R}  |  def456|api-gateway
  ${MG}▸${R} ${GR}rate-limit-config${R}          ${DG}api-gateway${R}       ${WH}apply stricter rate limits on auth endpoints. 5 attempts per minute per IP${R}  |  ghi012|api-gateway
  ${MG}▸${R} ${B}${GN}payment-gateway${R}            ${DG}my-saas-app${R}       ${WH}stripe webhook needs auth verification to prevent replay attacks${R}  |  jkl345|my-saas-app
  ${MG}▸${R} ${GR}compliance-audit${R}           ${DG}architex${R}          ${WH}legal flagged the auth middleware for not meeting the new session token standards${R}  |  mno678|architex")

selected=$(echo -e "$results" | fzf \
  --header=$'  \033[38;5;176m\033[1mclaude-picker\033[0m  \033[38;5;242m│  search: auth  │  7 matches in 3 projects\033[0m\n' \
  --header-first \
  --delimiter="|" \
  --with-nth=1 \
  --layout=reverse \
  --height=85% \
  --border=rounded \
  --margin=1,3 \
  --padding=1,1 \
  --prompt="  search > " \
  --pointer=" ▸" \
  --color="fg:249,fg+:114,bg+:-1,hl:222,hl+:222,pointer:176,prompt:117,header:176,border:238,gutter:-1" \
  --no-separator \
  --query="auth" \
  --preview-window=hidden \
  --ansi)

if [ -n "$selected" ]; then
  sid=$(echo "$selected" | awk -F'|' '{print $(NF-1)}' | xargs)
  proj=$(echo "$selected" | awk -F'|' '{print $NF}' | xargs)
  echo ""
  echo -e "  ${GN}${B}Found in project: ${proj}${R}"
  echo -e "  ${DG}cd /Users/anshul/projects/${proj}${R}"
  echo -e "  ${DG}claude --resume ${sid}${R}"
  echo ""
fi

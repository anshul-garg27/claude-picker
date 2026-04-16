#!/bin/bash
# Mockup of what `claude --resume` shows (the problem we're solving).

R='\033[0m'; B='\033[1m'; D='\033[2m'
CY='\033[38;5;117m'; GN='\033[38;5;114m'; YL='\033[38;5;222m'
MG='\033[38;5;176m'; DG='\033[38;5;242m'; GR='\033[38;5;249m'
RD='\033[38;5;210m'

echo ""
echo -e "  ${GR}\$ claude --resume${R}"
echo ""
echo -e "  ${GR}? Pick a conversation to resume${R}"
echo ""
echo -e "  ${GR}  4a2e8f1c-9b3d-4e7a-a891-2f6c9d1e4b5a ${DG}(2 hours ago)${R}"
echo -e "  ${GR}  b7c9d2e0-1f4a-8b6c-d5e9-3a8f1c7b2d4e ${DG}(3 hours ago)${R}"
echo -e "  ${GR}  e5f8a3b1-7c2d-9e0f-b4a5-8d1c6f2e9b7a ${DG}(yesterday)${R}"
echo -e "  ${GR}  c2d6e1f7-3b9a-5c4d-e8f1-2a7b6d3c9e4f ${DG}(yesterday)${R}"
echo -e "  ${GR}  a9b3c7d2-8e4f-1a6b-c5d9-3e8f2c7a4b1d ${DG}(2 days ago)${R}"
echo -e "  ${GR}  f4e8b1c6-2d9a-7f3e-a5b8-1c9d4e2f7b6a ${DG}(3 days ago)${R}"
echo -e "  ${GR}  d1c5e9a3-4f8b-6c2d-e7a1-9b5f3c8e2d4a ${DG}(5 days ago)${R}"
echo ""
echo -e "  ${DG}...14 more${R}"
echo ""
echo -e "  ${RD}${D}no project names. no preview. no search.${R}"
echo -e "  ${RD}${D}which one had the auth fix?${R}"
echo ""

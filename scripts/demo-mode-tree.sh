#!/bin/bash
# Hardcoded demo of claude-picker --tree output, for GIF recording.

R='\033[0m'; B='\033[1m'; D='\033[2m'
CY='\033[38;5;117m'; GN='\033[38;5;114m'; YL='\033[38;5;222m'
MG='\033[38;5;176m'; DG='\033[38;5;242m'; GR='\033[38;5;249m'

echo ""
echo -e "  ${MG}${B}claude-picker${R}  ${DG}session tree${R}"
echo ""
echo -e "  ${DG}${D}────────────────────────────────────────────────────────${R}"
echo ""
echo -e "  ${B}${CY}architex${R}  ${DG}/Users/anshul/projects/architex${R}"
echo ""
echo -e "  ${DG}├─${R} ${YL}●${R} ${B}${GN}auth-refactor${R}         ${DG}5m ago   45 msgs${R}"
sleep 0.15
echo -e "  ${DG}│  └─${R} ${YL}◆${R} ${GN}auth-refactor-v2${R}  ${DG}3m ago   12 msgs${R}  ${MG}${D}forked${R}"
sleep 0.15
echo -e "  ${DG}├─${R} ${YL}●${R} ${B}${GN}drizzle-migration${R}     ${DG}1d ago   67 msgs${R}"
sleep 0.15
echo -e "  ${DG}│  ├─${R} ${YL}◆${R} ${GN}drizzle-rollback${R}   ${DG}1d ago   22 msgs${R}  ${MG}${D}forked${R}"
sleep 0.15
echo -e "  ${DG}│  └─${R} ${YL}◆${R} ${GN}drizzle-seed-data${R}  ${DG}22h ago  18 msgs${R}  ${MG}${D}forked${R}"
sleep 0.15
echo -e "  ${DG}├─${R} ${YL}●${R} ${B}${GN}fix-race-condition${R}    ${DG}2h ago   28 msgs${R}"
sleep 0.15
echo -e "  ${DG}├─${R} ${DG}○${R} ${GR}can you help me debug${R}  ${DG}4h ago   12 msgs${R}"
sleep 0.15
echo -e "  ${DG}└─${R} ${DG}○${R} ${GR}what's the best way to${R} ${DG}1d ago    6 msgs${R}"
echo ""
sleep 0.3
echo -e "  ${B}${CY}my-saas-app${R}  ${DG}/Users/anshul/projects/my-saas-app${R}"
echo ""
echo -e "  ${DG}├─${R} ${YL}●${R} ${B}${GN}payment-gateway${R}       ${DG}6h ago   34 msgs${R}"
sleep 0.1
echo -e "  ${DG}├─${R} ${YL}●${R} ${B}${GN}k8s-deployment${R}        ${DG}3d ago   54 msgs${R}"
sleep 0.1
echo -e "  ${DG}│  └─${R} ${YL}◆${R} ${GN}k8s-scaling${R}       ${DG}2d ago   19 msgs${R}  ${MG}${D}forked${R}"
sleep 0.1
echo -e "  ${DG}└─${R} ${DG}○${R} ${GR}fix the failing test${R}  ${DG}2d ago    8 msgs${R}"
echo ""
sleep 0.3
echo -e "  ${B}${CY}api-gateway${R}  ${DG}/Users/anshul/projects/api-gateway${R}"
echo ""
echo -e "  ${DG}├─${R} ${YL}●${R} ${B}${GN}rate-limiter${R}          ${DG}Apr 12   41 msgs${R}"
sleep 0.1
echo -e "  ${DG}└─${R} ${YL}●${R} ${B}${GN}middleware-refactor${R}   ${DG}Apr 10   29 msgs${R}"
echo ""
echo -e "  ${DG}${D}────────────────────────────────────────────────────────${R}"
echo ""
echo -e "  ${DG}${D}● named   ◆ forked   ○ unnamed${R}"
echo ""

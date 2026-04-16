#!/bin/bash
# Static mockup of the session picker (step 2 of claude-picker) with preview panel.

R='\033[0m'; B='\033[1m'; D='\033[2m'
CY='\033[38;5;117m'; GN='\033[38;5;114m'; YL='\033[38;5;222m'
MG='\033[38;5;176m'; DG='\033[38;5;242m'; GR='\033[38;5;249m'
WH='\033[97m'; BL='\033[38;5;111m'

echo ""
echo -e "  ${MG}${B}architex${R}  ${DG}enter open  │  ctrl-b pin  │  ctrl-e export  │  ctrl-d delete${R}"
echo ""
printf "  %-58s ${DG}│${R} ${GN}${B}auth-refactor${R}\n"                  "$(echo -e ${BL}■${R}  ${B}${GN}ui-redesign${R}                          ${DG}  5m ago${R}  ${DG}14 msgs${R})"
printf "  %-58s ${DG}│${R} ${DG}created  2026-04-16 14:30${R}\n"           "$(echo -e ${DG}  ── saved ──────────────────────────${R})"
printf "  %-58s ${DG}│${R} ${DG}messages 45${R}\n"                          "$(echo -e ${MG}▸${R} ${YL}●${R}  ${B}${GN}auth-refactor${R}                      ${DG}  2h ago${R}  ${DG}45 msgs${R})"
printf "  %-58s ${DG}│${R} ${DG}${D}────────────────────${R}\n"             "$(echo -e ${R}   ${YL}●${R}  ${B}${GN}fix-race-condition${R}                 ${DG}  3h ago${R}  ${DG}28 msgs${R})"
printf "  %-58s ${DG}│${R}\n"                                               "$(echo -e ${R}   ${YL}●${R}  ${B}${GN}drizzle-migration${R}                  ${DG}  1d ago${R}  ${DG}67 msgs${R})"
printf "  %-58s ${DG}│${R} ${CY}${B}you${R}  ${GR}the auth middleware${R}\n" "$(echo -e ${R}   ${YL}●${R}  ${B}${GN}mcp-postgres-setup${R}                 ${DG}  3d ago${R}  ${DG}12 msgs${R})"
printf "  %-58s ${DG}│${R} ${GR}is storing session tokens${R}\n"            "$(echo -e ${DG}  ── recent ─────────────────────────${R})"
printf "  %-58s ${DG}│${R} ${GR}in a way that doesn${YL}\'${GR}t meet${R}\n"  "$(echo -e ${R}   ${DG}○${R}  ${GR}fix the failing checkout test${R}      ${DG}  4h ago${R}  ${DG}31 msgs${R})"
printf "  %-58s ${DG}│${R} ${GR}compliance${R}\n"                           "$(echo -e ${R}   ${DG}○${R}  ${GR}refactor the payment processing${R}    ${DG}  1d ago${R}  ${DG} 8 msgs${R})"
printf "  %-58s ${DG}│${R}\n"                                               "$(echo -e ${R}   ${DG}○${R}  ${GR}add rate limiting to the API${R}       ${DG}  2d ago${R}  ${DG}19 msgs${R})"
printf "  %-58s ${DG}│${R} ${YL}ai${R}  ${WH}I${YL}\'${WH}ll restructure the${R}\n"  "$(echo -e ${R}   ${DG}○${R}  ${GR}${D}debug the websocket timeout${R}       ${DG}${D} Apr 14${R}  ${DG}${D} 6 msgs${R})"
printf "  %-58s ${DG}│${R} ${WH}session token storage to${R}\n" ""
printf "  %-58s ${DG}│${R} ${WH}use encrypted HttpOnly${R}\n"  "$(echo -e ${R}   ${DG}○${R}  ${GR}${D}kubernetes deployment yaml${R}         ${DG}${D} Apr 12${R}  ${DG}${D}22 msgs${R})"
printf "  %-58s ${DG}│${R} ${WH}cookies with rotation${R}\n" ""
echo ""
echo -e "  ${CY}  session >${R}"
echo ""

#!/bin/bash
# Static mockup of the project picker (step 1 of claude-picker).

R='\033[0m'; B='\033[1m'; D='\033[2m'
CY='\033[38;5;117m'; GN='\033[38;5;114m'; YL='\033[38;5;222m'
MG='\033[38;5;176m'; DG='\033[38;5;242m'; GR='\033[38;5;249m'

echo ""
echo -e "  ${MG}${B}claude-picker${R}  ${DG}147 sessions · 312 MB · ~\$36.12${R}"
echo ""
echo -e "   ${MG}▸${R} ${CY}${B}architex${R}  ${DG}main${R}           ${DG} just now${R}  ${GN}${D}████████████${R} ${DG}14 sessions${R}"
echo -e "      ${CY}${B}my-saas-app${R}  ${DG}payment-flow${R}  ${DG}   3h ago${R}  ${GN}${D}███████     ${R} ${DG} 7 sessions${R}"
echo -e "      ${CY}${B}api-gateway${R}  ${DG}main${R}          ${DG}   5h ago${R}  ${GN}${D}█████       ${R} ${DG} 5 sessions${R}"
echo -e "      ${CY}${B}infra-automation${R}  ${DG}terraform${R} ${DG}   1d ago${R}  ${GN}${D}████        ${R} ${DG} 4 sessions${R}"
echo -e "      ${CY}${B}dotfiles${R}  ${DG}main${R}              ${DG}   2d ago${R}  ${GN}${D}███         ${R} ${DG} 3 sessions${R}"
echo -e "      ${CY}${B}portfolio-site${R}  ${DG}redesign${R}   ${DG}  Apr 12${R}  ${GN}${D}██          ${R} ${DG} 2 sessions${R}"
echo -e "      ${CY}${B}ml-experiments${R}  ${DG}main${R}       ${DG}  Apr 10${R}  ${GN}${D}██          ${R} ${DG} 2 sessions${R}"
echo ""
echo -e "  ${CY}  project >${R}"
echo ""

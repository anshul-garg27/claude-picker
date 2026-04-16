#!/bin/bash
# Hardcoded demo of claude-picker --stats output, for GIF recording.

R='\033[0m'; B='\033[1m'; D='\033[2m'
CY='\033[38;5;117m'; GN='\033[38;5;114m'; YL='\033[38;5;222m'
MG='\033[38;5;176m'; DG='\033[38;5;242m'; GR='\033[38;5;249m'
PC='\033[38;5;216m'

echo ""
echo -e "  ${MG}${B}claude-picker${R}  ${DG}stats${R}"
echo ""
echo -e "  ${DG}${D}────────────────────────────────────────────────────────${R}"
echo ""
echo -e "  ${GR}total sessions${R}     ${B}${CY}147${R}"
echo -e "  ${GR}total tokens${R}       ${B}${YL}~2.4M${R}  ${DG}(estimated)${R}"
echo -e "  ${GR}total cost${R}         ${B}${GN}~\$36.12${R}  ${DG}(blended rate)${R}"
echo -e "  ${GR}disk usage${R}         ${B}${PC}312 MB${R}  ${DG}in ~/.claude/${R}"
echo ""
echo -e "  ${DG}${D}────────────────────────────────────────────────────────${R}"
echo ""
sleep 0.3
echo -e "  ${B}${MG}by project${R}"
echo ""
echo -e "  ${CY}architex${R}              ${GN}${D}██████████████████████${R}  ${GR}58 sessions${R}  ${DG}~\$14.82${R}"
echo -e "  ${CY}my-saas-app${R}           ${GN}${D}█████████████         ${R}  ${GR}34 sessions${R}  ${DG} ~\$9.40${R}"
echo -e "  ${CY}api-gateway${R}           ${GN}${D}████████              ${R}  ${GR}22 sessions${R}  ${DG} ~\$5.13${R}"
echo -e "  ${CY}dotfiles${R}              ${GN}${D}████                  ${R}  ${GR}12 sessions${R}  ${DG} ~\$1.87${R}"
echo -e "  ${CY}infra-automation${R}      ${GN}${D}███                   ${R}  ${GR}11 sessions${R}  ${DG} ~\$2.44${R}"
echo -e "  ${CY}portfolio-site${R}        ${GN}${D}██                    ${R}  ${GR}10 sessions${R}  ${DG} ~\$2.46${R}"
echo ""
echo -e "  ${DG}${D}────────────────────────────────────────────────────────${R}"
echo ""
sleep 0.3
echo -e "  ${B}${MG}activity${R}"
echo ""
echo -e "  ${GR}today${R}          ${B}${GN}7 sessions${R}"
echo -e "  ${GR}this week${R}      ${B}${YL}28 sessions${R}"
echo -e "  ${GR}older${R}          ${DG}112 sessions${R}"
echo ""
echo -e "  ${DG}${D}────────────────────────────────────────────────────────${R}"
echo ""
sleep 0.3
echo -e "  ${B}${MG}top sessions by tokens${R}"
echo ""
echo -e "  ${YL}●${R} ${B}${GN}debug-websockets${R}         ${DG}architex${R}       ${B}${YL}72.4k${R}  ${DG}~\$1.09${R}"
echo -e "  ${YL}●${R} ${B}${GN}drizzle-migration${R}        ${DG}architex${R}       ${B}${YL}67.2k${R}  ${DG}~\$1.01${R}"
echo -e "  ${YL}●${R} ${B}${GN}k8s-deployment${R}           ${DG}my-saas-app${R}    ${B}${YL}54.8k${R}  ${DG}~\$0.82${R}"
echo -e "  ${YL}●${R} ${B}${GN}auth-refactor${R}            ${DG}architex${R}       ${B}${YL}45.1k${R}  ${DG}~\$0.68${R}"
echo -e "  ${YL}●${R} ${B}${GN}payment-gateway${R}          ${DG}api-gateway${R}    ${B}${YL}38.3k${R}  ${DG}~\$0.57${R}"
echo ""

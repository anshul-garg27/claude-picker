#!/bin/bash
# Hardcoded demo of claude-picker --diff output, for GIF recording.

R='\033[0m'; B='\033[1m'; D='\033[2m'
CY='\033[38;5;117m'; GN='\033[38;5;114m'; YL='\033[38;5;222m'
MG='\033[38;5;176m'; DG='\033[38;5;242m'; GR='\033[38;5;249m'
WH='\033[97m'; PC='\033[38;5;216m'

echo ""
echo -e "  ${MG}${B}claude-picker${R}  ${DG}diff${R}"
echo ""
echo -e "  ${B}${GN}auth-refactor${R}  ${DG}vs${R}  ${B}${GN}auth-refactor-v2${R}  ${DG}${D}(forked)${R}"
echo ""
echo -e "  ${DG}${D}────────────────────────────────────────────────────────────────────────────${R}"
echo ""
sleep 0.2
echo -e "  ${B}${MG}common topics${R}"
echo -e "    ${GN}●${R} ${WH}session tokens${R}      ${GN}●${R} ${WH}HttpOnly cookies${R}     ${GN}●${R} ${WH}refresh flow${R}"
echo -e "    ${GN}●${R} ${WH}rate limiting${R}       ${GN}●${R} ${WH}JWT validation${R}"
echo ""
sleep 0.2
echo -e "  ${B}${MG}unique to${R} ${B}${GN}auth-refactor${R}"
echo -e "    ${YL}◆${R} ${GR}Redis rate limiter${R}"
echo -e "    ${YL}◆${R} ${GR}CSRF tokens${R}"
echo -e "    ${YL}◆${R} ${GR}SameSite cookie policy${R}"
echo ""
sleep 0.2
echo -e "  ${B}${MG}unique to${R} ${B}${GN}auth-refactor-v2${R}"
echo -e "    ${PC}◆${R} ${GR}device fingerprinting${R}"
echo -e "    ${PC}◆${R} ${GR}refresh token rotation${R}"
echo -e "    ${PC}◆${R} ${GR}oauth2 provider hooks${R}"
echo ""
echo -e "  ${DG}${D}────────────────────────────────────────────────────────────────────────────${R}"
echo ""
sleep 0.2
printf "  %-40s %-40s\n" "$(echo -e ${B}${GN}auth-refactor${R})" "$(echo -e ${B}${GN}auth-refactor-v2${R})"
echo -e "  ${DG}45 msgs · 5m ago${R}                          ${DG}12 msgs · 3m ago${R}"
echo ""
echo -e "  ${CY}${B}you${R} ${GR}the auth middleware is${R}          ${CY}${B}you${R} ${GR}let's make the refresh token${R}"
echo -e "  ${GR}storing session tokens in a way${R}          ${GR}rotation actually secure. we need${R}"
echo -e "  ${GR}that doesn't meet compliance${R}             ${GR}device binding${R}"
echo ""
sleep 0.15
echo -e "  ${YL}ai${R} ${WH}I'll restructure the session${R}          ${YL}ai${R} ${WH}Good call. I'll add device${R}"
echo -e "  ${WH}token storage to use encrypted${R}           ${WH}fingerprinting using a combination${R}"
echo -e "  ${WH}HttpOnly cookies${R}                         ${WH}of user agent + IP + ...${R}"
echo ""
sleep 0.15
echo -e "  ${CY}${B}you${R} ${GR}also need to handle the${R}          ${CY}${B}you${R} ${GR}and rotate the refresh token${R}"
echo -e "  ${GR}refresh token flow${R}                       ${GR}on every use${R}"
echo ""

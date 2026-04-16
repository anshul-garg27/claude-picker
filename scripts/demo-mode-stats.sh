#!/bin/bash
# Hardcoded demo of claude-picker --stats for GIF recording.
# Mirrors the real session-stats.py layout with fixed data for reproducible demos.

R='\033[0m'; B='\033[1m'; D='\033[2m'
TEXT='\033[38;5;253m'; SUB='\033[38;5;244m'; LINE='\033[38;5;238m'
MAUVE='\033[38;5;141m'; GREEN='\033[38;5;114m'; YELLOW='\033[38;5;222m'
BLUE='\033[38;5;111m'; PEACH='\033[38;5;215m'; RED='\033[38;5;210m'
TEAL='\033[38;5;116m'; PINK='\033[38;5;217m'

echo ""
# Header row
printf "  ${MAUVE}${B}claude-picker --stats${R}%*s${SUB}last 30 days · all projects${R}\n" 60 ""
echo ""

# KPI card row — tokens / cost / sessions (each 30 chars wide)
printf "  ${LINE}${D}╭─ ${R}${SUB}tokens${R} ${LINE}${D}────────────────────${R}  "
printf "${LINE}${D}╭─ ${R}${SUB}cost${R} ${LINE}${D}──────────────────────${R}  "
printf "${LINE}${D}╭─ ${R}${SUB}sessions${R} ${LINE}${D}──────────────────${R}\n"

printf "  ${TEXT}${B}14.2M${R}   ${TEAL}▁▂▃▄▅▆▇█${R}            "
printf "${GREEN}${B}\$127.48${R}  ${GREEN}▂▃▅▇█▆▇${R}             "
printf "${YELLOW}${B}847${R}    ${YELLOW}▆▅▇▆▇█${R}              \n"

printf "  ${SUB}8.1M input · 6.1M output${R}     "
printf "${SUB}avg \$4.25 / day${R}               "
printf "${SUB}62 named · 785 unnamed${R}       \n"

printf "  ${LINE}${D}╰─────────────────────────────${R}  "
printf "${LINE}${D}╰─────────────────────────────${R}  "
printf "${LINE}${D}╰─────────────────────────────${R}\n"

echo ""
# Per-project section
echo -e "  ${SUB}${D}── per project ──────────────────────────────────────────────────────────────────────────────${R}"
echo ""
sleep 0.2

printf "  ${GREEN}${B}architex        ${R}  ${GREEN}█████████████████████████████████████████████████${R}${LINE}░${R}  ${SUB}   \$47.20  ·   4.8M tok  ·  213 ses${R}\n"
printf "  ${TEAL}${B}ecommerce-api   ${R}  ${TEAL}█████████████████████████████████████████${R}${LINE}░░░░░░░░░${R}  ${SUB}   \$38.10  ·   3.9M tok  ·  187 ses${R}\n"
printf "  ${BLUE}${B}infra-automation${R}  ${BLUE}████████████████████████████${R}${LINE}░░░░░░░░░░░░░░░░░░░░░░${R}  ${SUB}   \$26.80  ·   2.6M tok  ·  142 ses${R}\n"
printf "  ${YELLOW}${B}portfolio-site  ${R}  ${YELLOW}████████████████${R}${LINE}░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░${R}  ${SUB}   \$15.30  ·   1.5M tok  ·   98 ses${R}\n"
printf "  ${PEACH}${B}claude-picker   ${R}  ${PEACH}████████${R}${LINE}░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░${R}  ${SUB}    \$8.20  ·   0.9M tok  ·   82 ses${R}\n"
printf "  ${PINK}${B}old-playground  ${R}  ${PINK}███${R}${LINE}░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░${R}  ${SUB}    \$3.10  ·   0.4M tok  ·   41 ses${R}\n"

echo ""
# Activity timeline — 30 day vertical bars
echo -e "  ${SUB}${D}── activity (30d) ───────────────────────────────────────────────────────────────────────────${R}"
echo ""
sleep 0.2

printf "    "
heights="▄▅▆▇▅▄▄▆█▄▄▆▃▅▆▅▅▅▄▄█▄▄▆▆▇▅▆▇█"
for i in $(seq 0 29); do
  ch="${heights:$i:1}"
  if [ "$i" = "8" ] || [ "$i" = "20" ]; then
    printf "${RED}${ch}${R} "
  elif [ "$i" = "29" ]; then
    printf "${GREEN}${B}${ch}${R} "
  else
    printf "${MAUVE}${ch}${R} "
  fi
done
echo ""

# Date labels — roughly every 5 days
printf "    "
printf "${SUB}Mar 17${R}         "
printf "${SUB}Mar 22${R}        "
printf "${SUB}Mar 27${R}         "
printf "${SUB}Apr 1${R}         "
printf "${SUB}Apr 6${R}         "
printf "${SUB}Apr 11${R}        "
printf "${SUB}Apr 16${R}\n"

# Annotation line (ouch at position 8, today at end)
printf "    "
printf "%*s${RED}${D}← ouch${R}%*s${GREEN}↑ today${R}\n" 17 "" 44 ""

echo ""
# Footer
echo -e "  ${SUB}press ${R}${TEXT}q${R}${SUB} to quit  ·  press ${R}${TEXT}e${R}${SUB} to export  ·  press ${R}${TEXT}t${R}${SUB} to toggle days/weeks${R}"
echo ""

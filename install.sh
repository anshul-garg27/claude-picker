#!/bin/bash
# claude-picker installer
# https://github.com/anshul-garg27/claude-picker

set -e

REPO="https://github.com/anshul-garg27/claude-picker.git"
INSTALL_DIR="$HOME/.claude-picker"
BIN_DIR="$HOME/.local/bin"

# Colors
R='\033[0m'; B='\033[1m'; D='\033[2m'
CY='\033[38;5;117m'; GN='\033[38;5;114m'; MG='\033[38;5;176m'
DG='\033[38;5;242m'; RD='\033[38;5;203m'

echo ""
echo -e "  ${MG}${B}claude-picker${R}  ${DG}installer${R}"
echo -e "  ${DG}Find, preview, and resume your Claude Code sessions${R}"
echo ""

# Check dependencies
missing=""
command -v python3 >/dev/null 2>&1 || missing="python3"
command -v fzf >/dev/null 2>&1 || missing="$missing fzf"
command -v claude >/dev/null 2>&1 || missing="$missing claude"

if [ -n "$missing" ]; then
  echo -e "  ${RD}Missing dependencies:${R} $missing"
  echo ""
  if echo "$missing" | grep -q "fzf"; then
    echo -e "  ${DG}Install fzf:${R}  brew install fzf  ${DG}(macOS)${R}"
    echo -e "               apt install fzf   ${DG}(Ubuntu/Debian)${R}"
  fi
  if echo "$missing" | grep -q "claude"; then
    echo -e "  ${DG}Install Claude Code:${R}  npm install -g @anthropic-ai/claude-code"
  fi
  echo ""
  exit 1
fi

# Detect if running from inside the repo itself
SCRIPT_REAL_DIR="$(cd "$(dirname "$0")" && pwd)"

if [ "$SCRIPT_REAL_DIR" != "$INSTALL_DIR" ]; then
  # Running from a different location — copy to install dir
  if [ -d "$INSTALL_DIR" ]; then
    rm -rf "$INSTALL_DIR"
  fi
  echo -e "  ${DG}Installing...${R}"
  cp -r "$SCRIPT_REAL_DIR" "$INSTALL_DIR"
elif [ -d "$INSTALL_DIR/.git" ]; then
  echo -e "  ${DG}Updating...${R}"
  cd "$INSTALL_DIR" && git pull --quiet
else
  echo -e "  ${DG}Already installed.${R}"
fi

# Make executable
chmod +x "$INSTALL_DIR/claude-picker"
chmod +x "$INSTALL_DIR/lib/"*.sh "$INSTALL_DIR/lib/"*.py 2>/dev/null

# Install Python rich for beautiful previews (optional but recommended)
if ! python3 -c "import rich" 2>/dev/null; then
  echo -e "  ${DG}Installing rich (for preview panel)...${R}"
  python3 -m pip install --user --quiet rich 2>/dev/null || true
fi

# Create bin directory and symlink
mkdir -p "$BIN_DIR"
ln -sf "$INSTALL_DIR/claude-picker" "$BIN_DIR/claude-picker"

# Check if bin is in PATH
if ! echo "$PATH" | tr ':' '\n' | grep -q "$BIN_DIR"; then
  SHELL_RC=""
  if [ -f "$HOME/.zshrc" ]; then
    SHELL_RC="$HOME/.zshrc"
  elif [ -f "$HOME/.bashrc" ]; then
    SHELL_RC="$HOME/.bashrc"
  fi

  if [ -n "$SHELL_RC" ]; then
    echo "export PATH=\"$BIN_DIR:\$PATH\"" >> "$SHELL_RC"
    echo -e "  ${DG}Added ${BIN_DIR} to PATH in ${SHELL_RC}${R}"
  fi
fi

# Warp integration (optional)
if [ -d "$HOME/.warp" ]; then
  mkdir -p "$HOME/.warp/tab_configs"
  cat > "$HOME/.warp/tab_configs/claude_picker.toml" << 'TOML'
name = "Claude Picker"
color = "magenta"

[[panes]]
id = "main"
type = "terminal"
commands = ["claude-picker"]
TOML
  echo -e "  ${GN}✓${R} Warp tab config installed ${DG}(available in + menu)${R}"
fi

# Shell keybinding (Ctrl+P to launch picker)
SHELL_RC=""
if [ -f "$HOME/.zshrc" ]; then
  SHELL_RC="$HOME/.zshrc"
elif [ -f "$HOME/.bashrc" ]; then
  SHELL_RC="$HOME/.bashrc"
fi

if [ -n "$SHELL_RC" ]; then
  if ! grep -q "claude-picker" "$SHELL_RC" 2>/dev/null; then
    cat >> "$SHELL_RC" << 'KEYBIND'

# claude-picker: Ctrl+P to browse Claude Code sessions
claude-picker-widget() { claude-picker; zle reset-prompt 2>/dev/null; }
if [ -n "$ZSH_VERSION" ]; then
  zle -N claude-picker-widget
  bindkey '^P' claude-picker-widget
fi
KEYBIND
    echo -e "  ${GN}✓${R} Keybinding installed ${DG}(Ctrl+P to launch)${R}"
  fi
fi

echo ""
echo -e "  ${GN}✓${R} Installed successfully!"
echo ""
echo -e "  ${CY}${B}Usage:${R}"
echo -e "  ${DG}\$${R} claude-picker              ${DG}# browse sessions${R}"
echo -e "  ${DG}\$${R} claude-picker --search      ${DG}# search across all conversations${R}"
echo -e "  ${DG}\$${R} Ctrl+P                      ${DG}# keybinding (after shell restart)${R}"
echo ""
echo -e "  ${DG}Warp users: click + → Claude Picker${R}"
echo ""

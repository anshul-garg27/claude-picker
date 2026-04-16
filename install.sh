#!/bin/sh
# claude-picker installer
# https://github.com/anshul-garg27/claude-picker
#
# Detects your platform, downloads a prebuilt Rust binary from GitHub
# Releases, and symlinks it into ~/.local/bin. Falls back to `cargo install`
# or the classic Python+fzf flow when no prebuilt binary is available.
#
# Usage:
#   curl --proto '=https' --tlsv1.2 -sSf https://claude-picker.dev/install.sh | sh
#   or: bash install.sh   (from a git clone)

set -eu

# ── config ────────────────────────────────────────────────────────────────
REPO_OWNER="anshul-garg27"
REPO_NAME="claude-picker"
REPO_SLUG="${REPO_OWNER}/${REPO_NAME}"
REPO_URL="https://github.com/${REPO_SLUG}"
RELEASES_API="https://api.github.com/repos/${REPO_SLUG}/releases/latest"

BIN_DIR="${HOME}/.local/bin"
SHARE_DIR="${HOME}/.local/share/claude-picker"
CLASSIC_DIR="${HOME}/.claude-picker"

# ── colors ────────────────────────────────────────────────────────────────
if [ -t 1 ] && [ "${NO_COLOR:-0}" = "0" ]; then
  R='\033[0m'; B='\033[1m'
  CY='\033[38;5;117m'; GN='\033[38;5;114m'; MG='\033[38;5;176m'
  DG='\033[38;5;242m'; RD='\033[38;5;203m'; YL='\033[38;5;222m'
else
  R=''; B=''; CY=''; GN=''; MG=''; DG=''; RD=''; YL=''
fi

say()  { printf '%b\n' "$*"; }
info() { say "  ${DG}$*${R}"; }
ok()   { say "  ${GN}✓${R} $*"; }
warn() { say "  ${YL}!${R} $*"; }
err()  { say "  ${RD}✗${R} $*" >&2; }
die()  { err "$*"; exit 1; }

# ── header ────────────────────────────────────────────────────────────────
say ""
say "  ${MG}${B}claude-picker${R}  ${DG}installer${R}"
say "  ${DG}Terminal session manager for Claude Code — written in Rust${R}"
say ""

# ── detect platform ───────────────────────────────────────────────────────
OS="$(uname -s 2>/dev/null || echo unknown)"
ARCH="$(uname -m 2>/dev/null || echo unknown)"
TARGET=""
case "${OS}-${ARCH}" in
  Darwin-arm64|Darwin-aarch64)      TARGET="aarch64-apple-darwin" ;;
  Darwin-x86_64)                    TARGET="x86_64-apple-darwin" ;;
  Linux-x86_64|Linux-amd64)         TARGET="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64|Linux-arm64)        TARGET="aarch64-unknown-linux-gnu" ;;
  *)                                TARGET="" ;;
esac

if [ -n "$TARGET" ]; then
  info "Platform: ${OS}/${ARCH} → ${TARGET}"
else
  warn "Unrecognised platform: ${OS}/${ARCH} — will try cargo fallback"
fi

# ── detect: running from inside a git clone? ──────────────────────────────
SCRIPT_SRC="$0"
if [ -f "$SCRIPT_SRC" ]; then
  SCRIPT_REAL_DIR="$(cd "$(dirname "$SCRIPT_SRC")" 2>/dev/null && pwd)"
else
  SCRIPT_REAL_DIR=""
fi

IN_REPO=0
if [ -n "$SCRIPT_REAL_DIR" ] \
   && [ -f "$SCRIPT_REAL_DIR/Cargo.toml" ] \
   && grep -q '^name = "claude-picker"' "$SCRIPT_REAL_DIR/Cargo.toml" 2>/dev/null; then
  IN_REPO=1
fi

# ── fetch helper ──────────────────────────────────────────────────────────
FETCH=""
if command -v curl >/dev/null 2>&1; then
  FETCH="curl --proto '=https' --tlsv1.2 -fLsS"
elif command -v wget >/dev/null 2>&1; then
  FETCH="wget -qO-"
fi

fetch() {
  # usage: fetch URL [DEST]
  _url="$1"; _dest="${2:-}"
  if [ -z "$FETCH" ]; then
    die "Neither curl nor wget is available."
  fi
  if [ -n "$_dest" ]; then
    if command -v curl >/dev/null 2>&1; then
      curl --proto '=https' --tlsv1.2 -fLsS -o "$_dest" "$_url"
    else
      wget -qO "$_dest" "$_url"
    fi
  else
    if command -v curl >/dev/null 2>&1; then
      curl --proto '=https' --tlsv1.2 -fLsS "$_url"
    else
      wget -qO- "$_url"
    fi
  fi
}

# ── install strategy chooser ──────────────────────────────────────────────
install_prebuilt() {
  # Pull the latest release tag from the GitHub API.
  info "Fetching latest release tag..."
  _json="$(fetch "$RELEASES_API" 2>/dev/null || true)"
  if [ -z "$_json" ]; then
    warn "Could not reach GitHub API."
    return 1
  fi
  _tag="$(printf '%s' "$_json" | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
  if [ -z "$_tag" ]; then
    warn "No release tag found on ${REPO_SLUG}."
    return 1
  fi
  info "Latest release: ${_tag}"

  _tarball="${REPO_NAME}-${TARGET}.tar.xz"
  _url="${REPO_URL}/releases/download/${_tag}/${_tarball}"
  _tmp="$(mktemp -d 2>/dev/null || mktemp -d -t cp-install)"
  trap 'rm -rf "$_tmp"' EXIT

  info "Downloading ${_tarball}..."
  if ! fetch "$_url" "${_tmp}/${_tarball}"; then
    warn "Download failed from ${_url}"
    return 1
  fi

  info "Extracting..."
  ( cd "$_tmp" && tar -xJf "$_tarball" ) || {
    warn "Failed to extract ${_tarball}"
    return 1
  }

  # The tarball contains a directory matching the stem of the archive.
  _stem="${_tarball%.tar.xz}"
  _extracted="${_tmp}/${_stem}"
  if [ ! -f "${_extracted}/${REPO_NAME}" ]; then
    # Some cargo-dist tarballs flatten to the archive root.
    _extracted="$_tmp"
  fi
  if [ ! -f "${_extracted}/${REPO_NAME}" ]; then
    warn "Binary not found inside tarball."
    return 1
  fi

  mkdir -p "${SHARE_DIR}/bin"
  install -m 0755 "${_extracted}/${REPO_NAME}" "${SHARE_DIR}/bin/${REPO_NAME}-rust"
  # Keep README/LICENSE next to the binary for reference.
  for aux in README.md LICENSE; do
    if [ -f "${_extracted}/${aux}" ]; then
      cp "${_extracted}/${aux}" "${SHARE_DIR}/${aux}"
    fi
  done

  mkdir -p "$BIN_DIR"
  ln -sf "${SHARE_DIR}/bin/${REPO_NAME}-rust" "${BIN_DIR}/${REPO_NAME}"
  ok "Installed prebuilt ${REPO_NAME} ${_tag} to ${BIN_DIR}/${REPO_NAME}"
  return 0
}

install_from_source() {
  # Preferred fallback when no prebuilt binary exists: cargo install.
  if ! command -v cargo >/dev/null 2>&1; then
    return 1
  fi
  if [ "$IN_REPO" = "1" ]; then
    info "Building from local checkout (cargo install --path .)..."
    ( cd "$SCRIPT_REAL_DIR" && cargo install --path . --locked --root "${HOME}/.local" ) \
      && ok "Installed ${REPO_NAME} to ${BIN_DIR}/${REPO_NAME}" && return 0
  else
    info "Building from git (cargo install --git ${REPO_URL})..."
    cargo install --git "${REPO_URL}.git" --locked --root "${HOME}/.local" ${REPO_NAME} \
      && ok "Installed ${REPO_NAME} to ${BIN_DIR}/${REPO_NAME}" && return 0
  fi
  return 1
}

install_classic() {
  warn "Falling back to the classic Python + fzf flow."
  warn "The new Rust binary is NOT installed — see ${REPO_URL} to try it later."
  say ""
  # Classic requires fzf, python3, claude.
  _missing=""
  command -v python3 >/dev/null 2>&1 || _missing="$_missing python3"
  command -v fzf     >/dev/null 2>&1 || _missing="$_missing fzf"
  command -v claude  >/dev/null 2>&1 || _missing="$_missing claude"
  if [ -n "$_missing" ]; then
    err "Classic mode needs:${_missing}"
    say "    fzf:    brew install fzf   (macOS) | apt install fzf (Debian)"
    say "    claude: npm install -g @anthropic-ai/claude-code"
    return 1
  fi

  if [ "$IN_REPO" = "1" ]; then
    _src="$SCRIPT_REAL_DIR"
  else
    if ! command -v git >/dev/null 2>&1; then
      err "git is required to bootstrap classic mode."
      return 1
    fi
    info "Cloning ${REPO_SLUG} to ${CLASSIC_DIR}..."
    rm -rf "$CLASSIC_DIR"
    git clone --depth 1 "${REPO_URL}.git" "$CLASSIC_DIR" >/dev/null 2>&1 \
      || { err "git clone failed."; return 1; }
    _src="$CLASSIC_DIR"
  fi

  # Install Python Rich (best-effort, used for pretty previews).
  if ! python3 -c "import rich" 2>/dev/null; then
    info "Installing python3 rich (for preview panel)..."
    python3 -m pip install --user --quiet rich 2>/dev/null || true
  fi

  mkdir -p "$BIN_DIR"
  # Point ~/.local/bin/claude-picker at the wrapper script so `--classic` works.
  ln -sf "${_src}/claude-picker" "${BIN_DIR}/claude-picker"
  ok "Classic mode installed — run with:  ${B}claude-picker --classic${R}"
  return 0
}

# ── main flow ─────────────────────────────────────────────────────────────
# If we're running inside a git clone, offer the fast path first.
if [ "$IN_REPO" = "1" ] && command -v cargo >/dev/null 2>&1; then
  info "Detected local clone with cargo available."
  info "Running: cargo install --path . --locked --root ~/.local"
  ( cd "$SCRIPT_REAL_DIR" && cargo install --path . --locked --root "${HOME}/.local" ) \
    && INSTALLED="source" || INSTALLED=""
fi

# Try prebuilt binary.
if [ -z "${INSTALLED:-}" ] && [ -n "$TARGET" ]; then
  install_prebuilt && INSTALLED="prebuilt" || true
fi

# Fall back to cargo install from git.
if [ -z "${INSTALLED:-}" ]; then
  install_from_source && INSTALLED="source" || true
fi

# Last resort: classic Python + fzf flow.
if [ -z "${INSTALLED:-}" ]; then
  install_classic && INSTALLED="classic" || die "All install paths failed."
fi

# ── PATH check ────────────────────────────────────────────────────────────
case ":$PATH:" in
  *":${BIN_DIR}:"*) ;;
  *)
    SHELL_RC=""
    [ -f "$HOME/.zshrc" ]  && SHELL_RC="$HOME/.zshrc"
    [ -z "$SHELL_RC" ] && [ -f "$HOME/.bashrc" ] && SHELL_RC="$HOME/.bashrc"
    if [ -n "$SHELL_RC" ] && ! grep -q "${BIN_DIR}" "$SHELL_RC" 2>/dev/null; then
      {
        printf '\n# Added by claude-picker installer\n'
        printf 'export PATH="%s:$PATH"\n' "$BIN_DIR"
      } >> "$SHELL_RC"
      ok "Added ${BIN_DIR} to PATH in ${SHELL_RC}"
    fi
    ;;
esac

# ── Warp tab config (preserve existing behavior) ──────────────────────────
if [ -d "$HOME/.warp" ]; then
  mkdir -p "$HOME/.warp/tab_configs"
  cat > "$HOME/.warp/tab_configs/claude_picker.toml" <<'TOML'
name = "Claude Picker"
color = "magenta"

[[panes]]
id = "main"
type = "terminal"
commands = ["claude-picker"]
TOML
  ok "Warp tab config installed (available in + menu)"
fi

# ── Ctrl+P shell keybinding (preserve existing behavior) ──────────────────
SHELL_RC=""
[ -f "$HOME/.zshrc" ]  && SHELL_RC="$HOME/.zshrc"
[ -z "$SHELL_RC" ] && [ -f "$HOME/.bashrc" ] && SHELL_RC="$HOME/.bashrc"

if [ -n "$SHELL_RC" ] && ! grep -q "claude-picker-widget" "$SHELL_RC" 2>/dev/null; then
  cat >> "$SHELL_RC" <<'KEYBIND'

# claude-picker: Ctrl+P to browse Claude Code sessions
claude-picker-widget() { claude-picker; zle reset-prompt 2>/dev/null; }
if [ -n "$ZSH_VERSION" ]; then
  zle -N claude-picker-widget
  bindkey '^P' claude-picker-widget
fi
KEYBIND
  ok "Ctrl+P keybinding installed in ${SHELL_RC}"
fi

# ── welcome ───────────────────────────────────────────────────────────────
say ""
say "  ${GN}${B}claude-picker installed.${R}"
say ""
say "  ${CY}claude-picker${R}                  browse projects and sessions"
say "  ${CY}claude-picker${R} ${DG}--stats${R}          dashboard with token cost"
say "  ${CY}claude-picker${R} ${DG}--tree${R}           session tree with forks"
say "  ${CY}claude-picker${R} ${DG}--help${R}           full help"
say ""
say "  ${DG}Ctrl+P in your shell launches it from anywhere.${R}"
say ""
if [ "${INSTALLED}" = "classic" ]; then
  say "  ${YL}Note:${R} you're on the classic Python + fzf fallback."
  say "         Run ${B}claude-picker --help${R} after installing Rust + cargo"
  say "         to switch to the native binary."
  say ""
fi

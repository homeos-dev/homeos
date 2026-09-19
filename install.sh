#!/usr/bin/env sh
# Install script for homeos
# Usage: curl -sSf https://raw.githubusercontent.com/homeos-dev/homeos/main/install.sh | sh

set -e

REPO="homeos-dev/homeos"
INSTALL_DIR="${HOMEOS_INSTALL_DIR:-$HOME/.local/bin}"

err() {
    echo "Error: $1" >&2
    exit 1
}

check_already_latest() {
    if [ -n "${HOMEOS_FORCE_INSTALL:-}" ]; then
        return 0
    fi
    if ! command -v homeos >/dev/null 2>&1; then
        return 0
    fi
    _ver_local="$(homeos --version 2>/dev/null | awk '{print $NF}')"
    if [ -z "$_ver_local" ]; then
        return 0
    fi
    _ver_response="$(curl -fsSL --max-time 5 "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null)" || return 0
    _ver_latest="$(printf '%s' "$_ver_response" | sed -n 's/.*"tag_name":[[:space:]]*"v\{0,1\}\([^"]*\)".*/\1/p' | head -n 1)"
    if [ -z "$_ver_latest" ]; then
        return 0
    fi
    if [ "$_ver_local" = "$_ver_latest" ]; then
        echo "homeos $_ver_local is already the latest. Set HOMEOS_FORCE_INSTALL=1 to reinstall."
        exit 0
    fi
}

check_already_latest

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS-$ARCH" in
    Linux-x86_64)   TARGET="x86_64-unknown-linux-gnu" ;;
    Linux-aarch64)  TARGET="aarch64-unknown-linux-gnu" ;;
    Darwin-x86_64)  TARGET="x86_64-apple-darwin" ;;
    Darwin-arm64)   TARGET="aarch64-apple-darwin" ;;
    *) err "Unsupported platform: $OS $ARCH" ;;
esac

command -v curl >/dev/null 2>&1 || err "curl is required but not installed"
command -v tar >/dev/null 2>&1 || err "tar is required but not installed"

URL="https://github.com/$REPO/releases/latest/download/homeos-$TARGET.tar.gz"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Downloading homeos for $TARGET..."
curl -fsSL "$URL" -o "$TMP/homeos.tar.gz" || err "download failed: $URL"

echo "Extracting..."
tar -xzf "$TMP/homeos.tar.gz" -C "$TMP" || err "extract failed"

mkdir -p "$INSTALL_DIR"
mv "$TMP/homeos" "$INSTALL_DIR/homeos"
chmod +x "$INSTALL_DIR/homeos"

echo "Installed homeos to $INSTALL_DIR/homeos"

SHELL_NAME="$(basename "${SHELL:-}")"
case "$SHELL_NAME" in
    bash)
        COMP_DIR="$HOME/.local/share/bash-completion/completions"
        COMP_FILE="$COMP_DIR/homeos"
        mkdir -p "$COMP_DIR"
        "$INSTALL_DIR/homeos" completion bash > "$COMP_FILE"
        echo ""
        echo "Installed bash completion to $COMP_FILE"
        echo "If bash-completion is installed, completion will be available in new shells."
        ;;
    zsh)
        COMP_DIR="$HOME/.local/share/zsh/site-functions"
        COMP_FILE="$COMP_DIR/_homeos"
        mkdir -p "$COMP_DIR"
        "$INSTALL_DIR/homeos" completion zsh > "$COMP_FILE"
        echo ""
        echo "Installed zsh completion to $COMP_FILE"
        echo "Add the following to your ~/.zshrc (before 'compinit') if not already present:"
        echo ""
        echo "    fpath=($COMP_DIR \$fpath)"
        echo ""
        ;;
    fish)
        COMP_DIR="$HOME/.config/fish/completions"
        COMP_FILE="$COMP_DIR/homeos.fish"
        mkdir -p "$COMP_DIR"
        "$INSTALL_DIR/homeos" completion fish > "$COMP_FILE"
        echo ""
        echo "Installed fish completion to $COMP_FILE"
        ;;
    elvish)
        COMP_DIR="$HOME/.config/elvish/lib"
        COMP_FILE="$COMP_DIR/homeos.elv"
        mkdir -p "$COMP_DIR"
        "$INSTALL_DIR/homeos" completion elvish > "$COMP_FILE"
        echo ""
        echo "Installed elvish completion to $COMP_FILE"
        echo "Add the following to your ~/.config/elvish/rc.elv:"
        echo ""
        echo "    use homeos"
        echo ""
        ;;
esac

case ":$PATH:" in
    *":$INSTALL_DIR:"*)
        echo ""
        "$INSTALL_DIR/homeos" --version
        ;;
    *)
        cat <<EOF

Note: $INSTALL_DIR is not in your PATH.
Add the following to your shell config (e.g., ~/.bashrc):

    export PATH="$INSTALL_DIR:\$PATH"

Then open a new terminal.
EOF
        ;;
esac

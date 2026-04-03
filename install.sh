#!/usr/bin/env sh
# Aeterna CLI installer
# Downloads the correct pre-built binary for the current OS/arch from GitHub Releases.
# Usage: curl -fsSL https://raw.githubusercontent.com/kikokikok/aeterna/main/install.sh | sh
# Or:    sh install.sh [--version v0.4.0] [--install-dir /usr/local/bin]

set -eu

REPO="kikokikok/aeterna"
BINARY="aeterna"
DEFAULT_INSTALL_DIR="${HOME}/.local/bin"

# ── Argument parsing ────────────────────────────────────────────────────────────
VERSION=""
INSTALL_DIR=""

while [ $# -gt 0 ]; do
  case "$1" in
    --version)  VERSION="$2";      shift 2 ;;
    --install-dir) INSTALL_DIR="$2"; shift 2 ;;
    *)
      echo "Unknown flag: $1" >&2
      echo "Usage: $0 [--version vX.Y.Z] [--install-dir PATH]" >&2
      exit 1
      ;;
  esac
done

# ── Platform detection ──────────────────────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)  OS_LABEL="linux"  ;;
  Darwin) OS_LABEL="macos"  ;;
  *)
    echo "Unsupported OS: $OS" >&2
    echo "Please build from source: https://github.com/${REPO}#installation" >&2
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64 | amd64) ARCH_LABEL="x86_64"  ;;
  arm64 | aarch64) ARCH_LABEL="aarch64" ;;
  *)
    echo "Unsupported architecture: $ARCH" >&2
    echo "Please build from source: https://github.com/${REPO}#installation" >&2
    exit 1
    ;;
esac

ARCHIVE="${BINARY}-${ARCH_LABEL}-${OS_LABEL}.tar.gz"

# ── Resolve version ─────────────────────────────────────────────────────────────
if [ -z "$VERSION" ]; then
  echo "Fetching latest release version..."
  if command -v curl >/dev/null 2>&1; then
    LATEST_JSON="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest")"
  elif command -v wget >/dev/null 2>&1; then
    LATEST_JSON="$(wget -qO- "https://api.github.com/repos/${REPO}/releases/latest")"
  else
    echo "Error: curl or wget is required" >&2
    exit 1
  fi
  VERSION="$(echo "$LATEST_JSON" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"
  if [ -z "$VERSION" ]; then
    echo "Error: could not determine latest release version" >&2
    exit 1
  fi
fi

echo "Installing ${BINARY} ${VERSION} (${ARCH_LABEL}-${OS_LABEL})..."

# ── Resolve install directory ────────────────────────────────────────────────────
if [ -z "$INSTALL_DIR" ]; then
  if [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="$DEFAULT_INSTALL_DIR"
    mkdir -p "$INSTALL_DIR"
  fi
fi

# ── Download and extract ─────────────────────────────────────────────────────────
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "Downloading from ${DOWNLOAD_URL}..."
if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$DOWNLOAD_URL" -o "${TMPDIR}/${ARCHIVE}"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "${TMPDIR}/${ARCHIVE}" "$DOWNLOAD_URL"
fi

tar -xzf "${TMPDIR}/${ARCHIVE}" -C "$TMPDIR"
chmod +x "${TMPDIR}/${BINARY}"
mv "${TMPDIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"

# ── Post-install ─────────────────────────────────────────────────────────────────
echo ""
echo "Installed: ${INSTALL_DIR}/${BINARY}"
echo ""

# Warn if INSTALL_DIR is not in PATH
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo "Note: Add ${INSTALL_DIR} to your PATH:"
    echo "  export PATH=\"\$PATH:${INSTALL_DIR}\""
    echo ""
    ;;
esac

echo "Get started:"
echo "  aeterna auth login          # Authenticate with your Aeterna server"
echo "  aeterna status              # Verify connection and context"
echo "  aeterna --help              # Show all commands"

#!/usr/bin/env sh
# [LAYER: UI]
# Frictionless installer for HEAV3NS-MICRO CLI & HEAV3NS-CODE.app (macOS/Linux).
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/CardSorting/HEAV3NS-MICRO/main/bin/install.sh | sh
#   sh bin/install.sh [--app] [--cli-only] [--prefix ~/.local/bin] [--version 1.0.0]

set -eu

REPO="CardSorting/HEAV3NS-MICRO"
APP_REPO="CardSorting/HEAV3NS-CODE-Releases"
PREFIX="${HEAV3NS_INSTALL_DIR:-$HOME/.local/bin}"
INSTALL_APP=0
CLI_ONLY=0
VERSION="${HEAV3NS_VERSION:-latest}"

while [ "$#" -gt 0 ]; do
  case "$1" in
    --app) INSTALL_APP=1 ;;
    --cli-only) CLI_ONLY=1 ;;
    --prefix) shift; PREFIX="${1:-$PREFIX}" ;;
    --version) shift; VERSION="${1:-latest}" ;;
    -h|--help)
      echo "Usage: install.sh [--app] [--cli-only] [--prefix <dir>] [--version <semver|latest>]"
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS/$ARCH" in
  Darwin/arm64) TARGET="darwin-arm64" ;;
  Darwin/x86_64) TARGET="darwin-x86_64" ;;
  Linux/x86_64) TARGET="linux-x86_64" ;;
  Linux/aarch64|Linux/arm64) TARGET="linux-arm64" ;;
  *) echo "Unsupported platform: $OS/$ARCH (use 'cargo install --git https://github.com/$REPO')" >&2; exit 1 ;;
esac

mkdir -p "$PREFIX"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

if [ "$VERSION" = "latest" ]; then
  BASE_URL="https://github.com/$REPO/releases/latest/download"
else
  BASE_URL="https://github.com/$REPO/releases/download/v${VERSION#v}"
fi

echo "==> Installing heav3ns-micro ($TARGET) into $PREFIX..."
ARCHIVE="heav3ns-micro-${VERSION#v}-$TARGET.tar.gz"
if [ "$VERSION" = "latest" ]; then
  ARCHIVE="heav3ns-micro-$TARGET.tar.gz"
fi

if curl -fsSL "$BASE_URL/$ARCHIVE" -o "$TMP_DIR/$ARCHIVE" 2>/dev/null; then
  if curl -fsSL "$BASE_URL/SHA256SUMS.txt" -o "$TMP_DIR/SHA256SUMS.txt" 2>/dev/null; then
    if grep -q "$ARCHIVE" "$TMP_DIR/SHA256SUMS.txt"; then
      (cd "$TMP_DIR" && grep "$ARCHIVE" SHA256SUMS.txt | shasum -a 256 -c -)
    fi
  fi
  tar -xzf "$TMP_DIR/$ARCHIVE" -C "$TMP_DIR"
  install -m 0755 "$TMP_DIR/heav3ns-micro" "$PREFIX/heav3ns-micro"
elif command -v cargo >/dev/null 2>&1; then
  echo "==> Prebuilt archive unavailable; building from source via cargo..."
  cargo install --git "https://github.com/$REPO.git" --locked
else
  echo "Neither prebuilt release archive nor cargo found." >&2
  exit 1
fi

if [ "$INSTALL_APP" -eq 1 ] && [ "$CLI_ONLY" -eq 0 ] && [ "$OS" = "Darwin" ]; then
  echo "==> Installing HEAV3NS-CODE.app to /Applications..."
  APP_URL="https://github.com/$APP_REPO/releases/latest/download/HEAV3NS-CODE-arm64.zip"
  curl -fsSL "$APP_URL" -o "$TMP_DIR/HEAV3NS-CODE.zip"
  ditto -xk "$TMP_DIR/HEAV3NS-CODE.zip" "$TMP_DIR/app"
  rm -rf /Applications/HEAV3NS-CODE.app
  ditto "$TMP_DIR/app/HEAV3NS-CODE.app" /Applications/HEAV3NS-CODE.app
  codesign --verify --deep --strict /Applications/HEAV3NS-CODE.app
fi

echo ""
echo "✔ Installed heav3ns-micro. Next steps:"
echo "  1. Ensure $PREFIX is on your PATH"
echo "  2. Run zero-key offline tour:  heav3ns-micro demo"
echo "  3. Configure provider keys:    heav3ns-micro init"

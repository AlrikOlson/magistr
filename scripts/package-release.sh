#!/usr/bin/env bash
# Build and package the CLI for the host platform.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

HOST_TRIPLE="$(rustc -vV | awk '/^host:/ { print $2 }')"
cargo build --locked --release -p magistr-cli --target "$HOST_TRIPLE" --target-dir "$ROOT/target"
BINARY="$ROOT/target/$HOST_TRIPLE/release/magistr"
RELEASE_VERSION="$("$BINARY" --version | awk '{ print $2 }')"
PACKAGE_NAME="magistr-${RELEASE_VERSION}-${HOST_TRIPLE}"
DIST="$ROOT/target/release-dist"
mkdir -p "$DIST"
STAGING="$(mktemp -d "$DIST/.magistr-package.XXXXXX")"
trap 'rm -rf "$STAGING"' EXIT
PACKAGE_DIR="$STAGING/$PACKAGE_NAME"
mkdir -p "$PACKAGE_DIR/completions" "$PACKAGE_DIR/man"
cp "$BINARY" "$PACKAGE_DIR/magistr"
cp LICENSE "$PACKAGE_DIR/LICENSE"

for completion_shell in bash zsh fish; do
    "$PACKAGE_DIR/magistr" completions "$completion_shell" > "$PACKAGE_DIR/completions/magistr.$completion_shell"
done
"$PACKAGE_DIR/magistr" man-pages "$PACKAGE_DIR/man"

tar -czf "$DIST/$PACKAGE_NAME.tar.gz" -C "$STAGING" "$PACKAGE_NAME"
cd "$DIST"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$PACKAGE_NAME.tar.gz" > "$PACKAGE_NAME.sha256"
else
    shasum -a 256 "$PACKAGE_NAME.tar.gz" > "$PACKAGE_NAME.sha256"
fi
printf '%s\n' "$DIST/$PACKAGE_NAME.tar.gz" "$DIST/$PACKAGE_NAME.sha256"

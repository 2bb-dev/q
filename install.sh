#!/bin/sh

set -eu

repo="2bb-dev/q"
install_dir="${Q_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
    printf 'q installer: %s\n' "$1" >&2
    exit 1
}

command -v curl >/dev/null 2>&1 || fail "curl is required"

os=$(uname -s)
arch=$(uname -m)

case "$os" in
    Darwin)
        platform="macos"
        ;;
    Linux)
        platform="linux"
        ;;
    MINGW* | MSYS* | CYGWIN*)
        platform="windows"
        ;;
    *)
        fail "unsupported operating system: $os"
        ;;
esac

case "$arch" in
    x86_64 | amd64)
        architecture="x86_64"
        ;;
    arm64 | aarch64)
        architecture="aarch64"
        ;;
    *)
        fail "unsupported architecture: $arch"
        ;;
esac

if [ "$platform" = "linux" ] && [ "$architecture" = "aarch64" ]; then
    fail "Linux aarch64 releases are not available yet"
fi
if [ "$platform" = "windows" ] && [ "$architecture" != "x86_64" ]; then
    fail "Windows $architecture releases are not available yet"
fi

if [ "$platform" = "windows" ]; then
    archive="q-windows-${architecture}.zip"
    binary="q.exe"
    command -v unzip >/dev/null 2>&1 || fail "unzip is required"
else
    archive="q-${platform}-${architecture}.tar.gz"
    binary="q"
    command -v tar >/dev/null 2>&1 || fail "tar is required"
fi
base_url="https://github.com/${repo}/releases/latest/download"
tmp_dir=$(mktemp -d)
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

printf 'Downloading %s...\n' "$archive"
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
    "$base_url/$archive" -o "$tmp_dir/$archive"
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
    "$base_url/SHA256SUMS" -o "$tmp_dir/SHA256SUMS"

expected=$(awk -v archive="$archive" '$2 == archive { print $1 }' "$tmp_dir/SHA256SUMS")
[ -n "$expected" ] || fail "checksum for $archive is missing"

if command -v sha256sum >/dev/null 2>&1; then
    actual=$(sha256sum "$tmp_dir/$archive" | awk '{ print $1 }')
elif command -v shasum >/dev/null 2>&1; then
    actual=$(shasum -a 256 "$tmp_dir/$archive" | awk '{ print $1 }')
else
    fail "sha256sum or shasum is required"
fi

[ "$actual" = "$expected" ] || fail "checksum verification failed"
printf 'Checksum verified.\n'

if [ "$platform" = "windows" ]; then
    unzip -q "$tmp_dir/$archive" -d "$tmp_dir"
else
    tar -xzf "$tmp_dir/$archive" -C "$tmp_dir"
fi
[ -f "$tmp_dir/$binary" ] || fail "release archive does not contain $binary"

mkdir -p "$install_dir"
install -m 0755 "$tmp_dir/$binary" "$install_dir/$binary"
printf 'Installed q to %s/%s\n' "$install_dir" "$binary"

case ":$PATH:" in
    *":$install_dir:"*) ;;
    *) printf 'Add %s to PATH to run q.\n' "$install_dir" ;;
esac

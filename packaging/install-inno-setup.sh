#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
version=7.1.1
checksum=8a5a6711c3766a6e5fb74f280ba0e3551e719ce94538d901fdbd1f8f9920b3f0
mkdir -p target/inno-setup
download="target/innosetup-$version-x64.exe"
curl --fail --location --retry 3 --output "$download" \
    "https://github.com/jrsoftware/issrc/releases/download/is-${version//./_}/innosetup-$version-x64.exe"
printf '%s  %s\n' "$checksum" "$download" | sha256sum --check
MSYS_NO_PATHCONV=1 "$download" /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- \
    /CURRENTUSER /PORTABLE=1 "/DIR=$(cygpath -w "$PWD/target/inno-setup")"
compiler="$PWD/target/inno-setup/ISCC.exe"
test -f "$compiler"
if [[ -n ${GITHUB_ENV:-} ]]; then
    printf 'ISCC=%s\n' "$(cygpath -m "$compiler")" >> "$GITHUB_ENV"
fi
printf 'Inno Setup compiler: %s\n' "$compiler"

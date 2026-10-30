#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
compiler=${ISCC:-'/c/Program Files (x86)/Inno Setup 6/ISCC.exe'}
if [[ ! -f "$compiler" ]]; then
    printf 'Inno Setup compiler not found: %s. Set ISCC to ISCC.exe.\n' "$compiler" >&2
    exit 1
fi
command -v cargo-bundle-licenses >/dev/null

RUSTFLAGS="${RUSTFLAGS:-} -C target-feature=+crt-static" cargo +stable build --release --bin circadia --features self-contained --locked --target x86_64-pc-windows-msvc
metadata=$(cargo +stable metadata --format-version 1 --features self-contained --locked)
version=$(jq -er '.packages[] | select(.name == "circadia") | .version' <<< "$metadata")
base_version=$version
case ${CIRCADIA_CHANNEL:-development} in
    stable) ;;
    preview) version="$version-preview" ;;
    development) version="$version-dev" ;;
    *) printf 'Invalid CIRCADIA_CHANNEL\n' >&2; exit 1 ;;
esac
manifest=$(jq -er '.packages[] | select(.name == "windows-reactor-setup") | .manifest_path' <<< "$metadata")
helper=$(dirname "${manifest//\//}")
runtime_version=$(sed -n 's/^const RUNTIME_VER: &str = "\([^"]*\)";.*/\1/p' "$helper/src/lib.rs")
webview_version=$(sed -n 's/^const WEBVIEW2_VER: &str = "\([^"]*\)";.*/\1/p' "$helper/src/lib.rs")
revision=$(git rev-parse --short=12 HEAD)
release=target/x86_64-pc-windows-msvc/release
mkdir -p target/installer
payload=$(mktemp -d "$PWD/target/installer/payload.XXXXXX")
cp "$release/circadia.exe" "$release/circadia.ico" "$release/Microsoft.Web.WebView2.Core.dll" LICENSE "$payload/"
while IFS= read -r entry || [[ -n "$entry" ]]; do
    entry=${entry%$'\r'}
    if [[ -n "$entry" && -e "$release/$entry" ]]; then
        cp -R "$release/$entry" "$payload/"
    fi
done < "$helper/assets/runtime.txt"
for required in Microsoft.WindowsAppRuntime.dll Microsoft.ui.xaml.dll resources.pri; do
    test -f "$payload/$required"
done
mkdir "$payload/licenses"
cp packaging/licenses/windows-app-sdk-"$runtime_version"-*.txt "$payload/licenses/"
cp packaging/licenses/webview2-"$webview_version"-*.txt "$payload/licenses/"
cargo bundle-licenses --format json --features self-contained --previous packaging/licenses/rust-overrides.json --output "$payload/licenses/rust.json"
jq -e 'all(.third_party_libraries[]; any(.licenses[]; .text != "NOT FOUND" and (.text | length > 0)))' "$payload/licenses/rust.json" >/dev/null
printf 'Circadia %s\nCommit: %s\nChannel: %s\nUnsigned build\n' "$version" "$(git rev-parse HEAD)" "${CIRCADIA_CHANNEL:-development}" > "$payload/BUILD.txt"

MSYS_NO_PATHCONV=1 "$compiler" "/DPayloadDir=$(cygpath -w "$payload")" "/DAppVersion=$version" "/DBaseVersion=$base_version" "/DBuildRevision=$revision" "/O$(cygpath -w "$PWD/target/installer")" packaging/circadia.iss
installer="Circadia-$version-$revision-windows-x64-unsigned-setup.exe"
(cd target/installer && sha256sum "$installer" > "$installer.sha256")
if [[ -n ${GITHUB_OUTPUT:-} ]]; then
    printf 'installer=target/installer/%s\n' "$installer" >> "$GITHUB_OUTPUT"
fi

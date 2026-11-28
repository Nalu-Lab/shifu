#!/bin/bash
# Build the Apple Silicon download package for a release tag, e.g. remote-v0.3.8-preview.1.
# Output: target/release-package.noindex/Shifu-remote-v<version>-apple-silicon.zip and SHA256SUMS; with an updater key
# (TAURI_SIGNING_PRIVATE_KEY or TAURI_SIGNING_PRIVATE_KEY_PATH, plus TAURI_SIGNING_PRIVATE_KEY_PASSWORD) also the in-app
# updater's Shifu-remote-v<version>-apple-silicon.app.tar.gz, its .sig and latest.json.
set -euo pipefail
tag=${1:?usage: package.sh <tag, e.g. remote-v0.3.8-preview.1>}
version=$(printf '%s' "$tag" | sed -E 's/^remote-v([0-9]+\.[0-9]+\.[0-9]+).*/\1/')
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "tag must look like remote-v1.2.3-...: $tag" >&2; exit 1; }
[ "$(uname -s)-$(uname -m)" = Darwin-arm64 ] || { echo 'Build on an Apple Silicon Mac.' >&2; exit 1; }
root=$(cd "$(dirname "$0")/../.." && pwd)
name="Shifu-remote-v$version-apple-silicon"
out="$root/target/release-package.noindex"
pkg="$out/$name"
rm -rf "$pkg" "$out/$name.zip" "$out/SHA256SUMS"
mkdir -p "$pkg/bin" "$pkg/helpers" "$pkg/scripts" "$pkg/docs"

# One fixed certificate for every release (imported by the release workflow) keeps macOS
# Accessibility and Screen Recording across updates; without it the build falls back to ad-hoc.
identity=${SHIFU_RELEASE_IDENTITY:--}
rm -f "$out"/*.app.tar.gz "$out"/*.app.tar.gz.sig "$out/latest.json"

# Computer Use helper first: Shifu.app carries a copy (Resources/helpers) so that an in-app update can refresh an
# installed helper too (app/src-tauri/src/update.rs). The bundle folder is emptied again afterwards, so a later
# local build does not pick up this copy.
SHIFU_SIGNING_IDENTITY="$identity" bash "$root/native/computer-use-macos/build-app.sh" >/dev/null
helpers_bundle="$root/app/src-tauri/helpers-bundle"
clear_helpers() { find "$helpers_bundle" -mindepth 1 -maxdepth 1 ! -name README.md -exec rm -rf {} +; }
trap clear_helpers EXIT
clear_helpers
ditto "$root/target/computer-use.noindex/Shifu Computer Use.app" "$helpers_bundle/Shifu Computer Use.app"

# Desktop app (prepare-desktop.sh, run by tauri, builds the frontend and the bundled remote relay).
APPLE_SIGNING_IDENTITY="$identity" npm --prefix "$root/app" run build:desktop -- --config "{\"version\":\"$version\"}"
# This checkout's own desktop cache: another worktree may be building at the same time.
cache=$(cd "$root" && python3 scripts/build-cache.py path --kind desktop)
app="$cache/release/bundle/macos/Shifu.app"
[ -d "$app" ] || app=""
[ -n "$app" ] || { echo 'Shifu.app not found after build' >&2; exit 1; }
ditto "$app" "$pkg/Shifu.app"
[ "$(/usr/libexec/PlistBuddy -c 'Print CFBundleShortVersionString' "$pkg/Shifu.app/Contents/Info.plist")" = "$version" ]
[ -x "$pkg/Shifu.app/Contents/Resources/helpers/Shifu Computer Use.app/Contents/MacOS/shifu-computer-use-macos" ]
# The bundled tmux runs with macOS's own libraries only (no Homebrew).
"$pkg/Shifu.app/Contents/Resources/bin/tmux" -V >/dev/null
if otool -L "$pkg/Shifu.app/Contents/Resources/bin/tmux" | tail -n +2 | awk '{print $1}' | grep -vE '^(/usr/lib/|/System/)' | grep -q .; then echo 'bundled tmux links outside macOS' >&2; exit 1; fi
/usr/bin/codesign --verify --deep --strict "$pkg/Shifu.app"
clear_helpers

# In-app updater package: the same signed Shifu.app as a .tar.gz (top folder Shifu.app, no macOS metadata files),
# signed with the updater key for this version, and the latest.json that names it.
if [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]; then
  updater="$out/$name.app.tar.gz"
  COPYFILE_DISABLE=1 tar --no-mac-metadata --no-xattrs -czf "$updater" -C "$pkg" Shifu.app
  "$root/app/node_modules/.bin/tauri" signer sign --app-version "$version" "$updater" >/dev/null
  python3 "$root/scripts/release/update-manifest.py" make --tag "$tag" --package "$updater" --signature "$updater.sig" --out "$out/latest.json"
else
  echo 'No updater key (TAURI_SIGNING_PRIVATE_KEY); building without the in-app update package.' >&2
fi

# Matching CLI for terminal use (the app already carries one, so installing it is optional) and optional Computer Use helper (signed with the same identity, not notarized).
cargo build --locked --release --manifest-path "$root/Cargo.toml" --bin shifu
install -m 755 "$root/target/release/shifu" "$pkg/bin/shifu"
# The same tmux as the app's, so the terminal CLI also works on a Mac without one.
install -m 755 "$pkg/Shifu.app/Contents/Resources/bin/tmux" "$pkg/bin/tmux"
# shifu web looks for web/cli.mjs next to the CLI; ship it with its one pinned dependency (pure JS).
mkdir -p "$pkg/bin/web"
cp "$root"/web/*.mjs "$root/web/package.json" "$root/web/package-lock.json" "$pkg/bin/web/"
npm --prefix "$pkg/bin/web" ci --ignore-scripts --omit=dev --no-audit --no-fund >/dev/null
ditto "$root/target/computer-use.noindex/Shifu Computer Use.app" "$pkg/helpers/Shifu Computer Use.app"

cp "$root/scripts/autopilot.py" "$pkg/scripts/"
for doc in AUTOPILOT BROWSER-USE BUILD-CACHE COMPUTER-USE INSTALL INSTALL.en REMOTE-RELAY REMOTE-RELAY.en STEPFUN WEB-POOL; do
  cp "$root/docs/$doc.md" "$pkg/docs/"
done
for lang in zh-CN en; do sed "s/{{VERSION}}/$version/g" "$root/scripts/release/INSTALL.$lang.md" > "$pkg/INSTALL.$lang.md"; done
cp "$root/LICENSE" "$root/THIRD_PARTY_NOTICES.md" "$pkg/"

commit=$(git -C "$root" rev-parse HEAD)
signature=ad-hoc
[ "$identity" != - ] && signature="certificate: $identity"
built_by=local
[ -n "${GITHUB_RUN_ID:-}" ] && built_by="GitHub Actions run $GITHUB_RUN_ID"
cat > "$pkg/BUILD-INFO.json" <<JSON
{
  "release_version": "$version",
  "release_tag": "$tag",
  "source_commit": "$commit",
  "source_tree": "$(git -C "$root" rev-parse 'HEAD^{tree}')",
  "platform": "macOS-arm64",
  "signature": "$signature",
  "notarized": false,
  "built_by": "$built_by",
  "built_at_utc": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "included": ["Shifu.app (with the matching CLI at Contents/Resources/bin/shifu and Computer Use helper at Contents/Resources/helpers)", "tmux at Contents/Resources/bin/tmux and bin/tmux (used when no tmux is installed)", "bin/shifu (optional, for terminal use)", "helpers/Shifu Computer Use.app", "scripts/autopilot.py", "public instructions and licenses"],
  "external_runtimes": ["chosen agents", "Node/Pi for StepFun", "browser runtime for Browser Use"]
}
JSON

(cd "$pkg" && find . -type f ! -name SHA256SUMS | sed 's#^\./##' | LC_ALL=C sort | tr '\n' '\0' | xargs -0 shasum -a 256 > SHA256SUMS)
# SHA256SUMS lists the zip only, so `shasum -c SHA256SUMS` next to a downloaded zip keeps working; the updater
# package carries its own signature, which the app checks before installing it.
(cd "$out" && ditto -c -k --keepParent --norsrc "$name" "$name.zip" && shasum -a 256 "$name.zip" > SHA256SUMS)
echo "$out/$name.zip"

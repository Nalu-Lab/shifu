#!/bin/bash
set -euo pipefail
project_root=$(cd "$(dirname "$0")/.." && pwd)
if [ -z "${CARGO_TARGET_DIR:-}" ] || [ ! -f "$CARGO_TARGET_DIR/.shifu-cache.json" ]; then
  echo '请使用 npm --prefix app run build:desktop -- <参数>；直接运行 tauri build 会在旧目录重复积累编译结果。' >&2
  exit 1
fi
build_dir=${SHIFU_REMOTE_BUILD_DIR:-"$project_root/remote/target"}
bundle="$project_root/app/src-tauri/remote-bundle"
npm --prefix "$project_root/app" run build:remote
npm --prefix "$project_root/app" run build
cargo build --locked --release --manifest-path "$project_root/remote/Cargo.toml" --target-dir "$build_dir"
# The app ships its matching CLI (Contents/Resources/bin/shifu): hooks, the 项目经理 and `shifu` in agents' PATH use it
# when no CLI was installed apart from the app.
cargo build --locked --release --manifest-path "$project_root/Cargo.toml" --bin shifu
mkdir -p "$project_root/app/src-tauri/cli-bundle"
install -m 755 "$CARGO_TARGET_DIR/release/shifu" "$project_root/app/src-tauri/cli-bundle/shifu.tmp"
mv -f "$project_root/app/src-tauri/cli-bundle/shifu.tmp" "$project_root/app/src-tauri/cli-bundle/shifu"
# tmux ships beside it (Contents/Resources/bin/tmux), so a Mac without tmux needs nothing else; Shifu prefers a tmux
# the person installed (src/tmux.rs). Built once from pinned sources, then reused (scripts/build-tmux.sh).
tmux_bin=$(bash "$project_root/scripts/build-tmux.sh" | tail -n 1)
install -m 755 "$tmux_bin" "$project_root/app/src-tauri/cli-bundle/tmux.tmp"
mv -f "$project_root/app/src-tauri/cli-bundle/tmux.tmp" "$project_root/app/src-tauri/cli-bundle/tmux"
mkdir -p "$project_root/app/src-tauri/licenses-bundle/tmux"
cp "$(dirname "$tmux_bin")"/licenses/* "$project_root/app/src-tauri/licenses-bundle/tmux/"
stage_root=${SHIFU_BUILD_STAGE_ROOT:-"$project_root/target/remote-bundle-staging"}
mkdir -p "$bundle" "$stage_root"
stage=$(mktemp -d "$stage_root/release.XXXXXX")
finish_stage() {
  result=$?
  trap - EXIT
  if [ "$result" -eq 0 ]; then
    rm -rf "$stage"
  else
    echo "构建未完成，保留本次暂存用于续跑：$stage" >&2
  fi
  exit "$result"
}
trap finish_stage EXIT
install -m 755 "$build_dir/release/shifu-remote" "$stage/shifu-remote"
cp -R "$project_root/remote/web-dist" "$stage/web"
if [ -d "$bundle/web" ]; then mv "$bundle/web" "$stage/previous-web"; fi
mv -f "$stage/shifu-remote" "$bundle/shifu-remote"
mv "$stage/web" "$bundle/web"

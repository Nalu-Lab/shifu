#!/bin/bash
# Build a self-contained arm64 tmux for Shifu.app (Contents/Resources/bin/tmux).
# libevent, utf8proc and jemalloc are linked statically; ncurses and libSystem come from macOS (/usr/lib).
# Output: target/tmux.noindex/tmux and target/tmux.noindex/licenses/; the last stdout line is the binary's path.
set -euo pipefail
[ "$(uname -s)-$(uname -m)" = Darwin-arm64 ] || { echo 'Build on an Apple Silicon Mac.' >&2; exit 1; }
root=$(cd "$(dirname "$0")/.." && pwd)

# Pinned sources. Change a version here and its SHA-256 together; the stamp makes the next run rebuild.
tmux_version=3.8
tmux_sha256=e79c699c7e949dccd0a4a125e17b8d1261e311b16979dbc8eb34542f3966d82e
libevent_version=2.1.12-stable
libevent_sha256=92e6de1be9ec176428fd2367677e61ceffc2ee1cb119035037a27d346b0403bb
utf8proc_version=2.12.0
utf8proc_sha256=a393fbef160835fb315bc3e91ba8d86f7a73a7cec9e6198b6c60b848b498bfeb
# tmux 3.7c+ wants jemalloc on macOS: it works around a macOS calloc bug (tmux issue 5385).
jemalloc_version=5.4.0
jemalloc_sha256=200776fac271093e7c2f21edd6d62657ecd2be578d9328633f2a86bfa6ef4f1d
stamp_line="tmux $tmux_version libevent $libevent_version utf8proc $utf8proc_version jemalloc $jemalloc_version macos 11.0 arm64 r2"

build="$root/target/tmux-build.noindex"
out="$root/target/tmux.noindex"
bin="$out/tmux"

# Nothing from Homebrew or the caller's environment may reach the compiler or linker.
export PATH=/usr/bin:/bin:/usr/sbin:/sbin
unset CPATH C_INCLUDE_PATH LIBRARY_PATH PKG_CONFIG_PATH PKG_CONFIG_LIBDIR PKG_CONFIG CPPFLAGS LDFLAGS LIBS \
  DYLD_LIBRARY_PATH DYLD_FALLBACK_LIBRARY_PATH SDKROOT CC CFLAGS
export MACOSX_DEPLOYMENT_TARGET=11.0
export CC=/usr/bin/clang
export CFLAGS="-arch arm64 -O2 -mmacosx-version-min=$MACOSX_DEPLOYMENT_TARGET"
export LDFLAGS="-arch arm64 -mmacosx-version-min=$MACOSX_DEPLOYMENT_TARGET"
jobs=$(sysctl -n hw.ncpu)

# Checks run on every call, also when the build is skipped.
check() {
  local libs
  libs=$(otool -L "$bin" | tail -n +2 | awk '{print $1}')
  if printf '%s\n' "$libs" | grep -vE '^(/usr/lib/|/System/)' | grep -q .; then
    echo "tmux links outside /usr/lib and /System:" >&2
    otool -L "$bin" >&2
    exit 1
  fi
  "$bin" -V >&2
}

if [ -x "$bin" ] && [ "$(cat "$out/.stamp" 2>/dev/null)" = "$stamp_line" ]; then
  check
  echo "$bin"
  exit 0
fi

# Download once and verify every tarball against its pinned hash.
mkdir -p "$build/downloads"
fetch() {
  local url=$1 sha=$2 file="$build/downloads/${1##*/}"
  if [ ! -f "$file" ] || [ "$(shasum -a 256 "$file" | awk '{print $1}')" != "$sha" ]; then
    curl -fsSL --retry 3 -o "$file.part" "$url"
    mv "$file.part" "$file"
  fi
  [ "$(shasum -a 256 "$file" | awk '{print $1}')" = "$sha" ] || { echo "SHA-256 mismatch: $file" >&2; exit 1; }
}
fetch "https://github.com/tmux/tmux/releases/download/$tmux_version/tmux-$tmux_version.tar.gz" "$tmux_sha256"
fetch "https://github.com/libevent/libevent/releases/download/release-$libevent_version/libevent-$libevent_version.tar.gz" "$libevent_sha256"
fetch "https://github.com/JuliaStrings/utf8proc/releases/download/v$utf8proc_version/utf8proc-$utf8proc_version.tar.gz" "$utf8proc_sha256"
fetch "https://github.com/jemalloc/jemalloc/releases/download/$jemalloc_version/jemalloc-$jemalloc_version.tar.bz2" "$jemalloc_sha256"

# Fresh source trees and a private prefix on every build.
src="$build/src"
prefix="$build/prefix"
rm -rf "$src" "$prefix"
mkdir -p "$src" "$prefix/include" "$prefix/lib"
for f in "tmux-$tmux_version" "libevent-$libevent_version" "utf8proc-$utf8proc_version"; do
  tar -xzf "$build/downloads/$f.tar.gz" -C "$src"
done
tar -xjf "$build/downloads/jemalloc-$jemalloc_version.tar.bz2" -C "$src"

# libevent: static only, no OpenSSL.
(
  cd "$src/libevent-$libevent_version"
  ./configure --prefix="$prefix" --disable-shared --enable-static --disable-openssl \
    --disable-samples --disable-libevent-regress --disable-debug-mode >"$build/libevent.log" 2>&1
  make -j"$jobs" >>"$build/libevent.log" 2>&1
  make install >>"$build/libevent.log" 2>&1
) || { echo "libevent build failed, see $build/libevent.log" >&2; exit 1; }

# utf8proc: only the static library and its header.
(
  cd "$src/utf8proc-$utf8proc_version"
  make -j"$jobs" CC="$CC" CFLAGS="$CFLAGS" libutf8proc.a >"$build/utf8proc.log" 2>&1
  install -m 644 utf8proc.h "$prefix/include/"
  install -m 644 libutf8proc.a "$prefix/lib/"
) || { echo "utf8proc build failed, see $build/utf8proc.log" >&2; exit 1; }

# jemalloc: static library and headers only (no shared library, no docs, no C++).
(
  cd "$src/jemalloc-$jemalloc_version"
  ./configure --prefix="$prefix" --disable-cxx --disable-doc >"$build/jemalloc.log" 2>&1
  make -j"$jobs" build_lib_static >>"$build/jemalloc.log" 2>&1
  make install_include install_lib_static >>"$build/jemalloc.log" 2>&1
) || { echo "jemalloc build failed, see $build/jemalloc.log" >&2; exit 1; }

# tmux: the *_CFLAGS/*_LIBS variables make configure skip pkg-config (there is none on this Mac).
# The static archives are named by path so no dylib of the same name can be picked instead.
# jemalloc must be force-loaded: it takes over malloc by registering a macOS malloc zone from a constructor that
# nothing references, so a plain static link drops it and tmux then frees system-malloc memory with jemalloc (crash).
(
  cd "$src/tmux-$tmux_version"
  ./configure --prefix=/usr/local --enable-utf8proc --enable-jemalloc --disable-shared \
    LIBEVENT_CFLAGS="-I$prefix/include" LIBEVENT_LIBS="$prefix/lib/libevent_core.a" \
    LIBUTF8PROC_CFLAGS="-I$prefix/include" LIBUTF8PROC_LIBS="$prefix/lib/libutf8proc.a" \
    JEMALLOC_CFLAGS="-I$prefix/include" JEMALLOC_LIBS="-Wl,-force_load,$prefix/lib/libjemalloc.a" \
    YACC=yacc >"$build/tmux.log" 2>&1
  make -j"$jobs" >>"$build/tmux.log" 2>&1
) || { echo "tmux build failed, see $build/tmux.log" >&2; exit 1; }

rm -rf "$out"
mkdir -p "$out/licenses"
install -m 755 "$src/tmux-$tmux_version/tmux" "$bin"
strip "$bin"
install -m 644 "$src/tmux-$tmux_version/COPYING" "$out/licenses/tmux-COPYING"
install -m 644 "$src/libevent-$libevent_version/LICENSE" "$out/licenses/libevent-LICENSE"
install -m 644 "$src/utf8proc-$utf8proc_version/LICENSE.md" "$out/licenses/utf8proc-LICENSE.md"
install -m 644 "$src/jemalloc-$jemalloc_version/COPYING" "$out/licenses/jemalloc-COPYING"
check
printf '%s\n' "$stamp_line" >"$out/.stamp"
echo "$bin"

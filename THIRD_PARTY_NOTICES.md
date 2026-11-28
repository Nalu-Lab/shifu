# Third-party notices

The pixel Claude mascot in `app/src/mascots.ts` follows the Clawd drawing in claude-desktop-buddy (https://github.com/anthropics/claude-desktop-buddy), used under the MIT License:

```
Copyright 2026 Anthropic, PBC.

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

---

The GIF assets in characters/bufo/ are from the community "bufo" emoji set (https://bufo.zone). They are third-party artwork redistributed for convenience and are not covered by the MIT license above; all rights remain with their original creators. See characters/bufo/README.md.
```

Claude, Clawd, Codex, Kimi, CodeBuddy and Antigravity are trademarks of their respective owners. Their marks are shown only to tell which agent a row belongs to. The unmodified Antigravity SVG in `app/src/assets/antigravity.svg` comes from its [official press assets](https://antigravity.google/press); the mark remains its owner's property and is not relicensed under this repository's MIT license.

## macOS computer use

`native/computer-use-macos` is adapted from [stablyai/orca](https://github.com/stablyai/orca/tree/d1466f6017c4bb6006b77eb46f2fc0f461ef021e/native/computer-use-macos), commit `d1466f6017c4bb6006b77eb46f2fc0f461ef021e`. Copyright Lovecast, Inc., MIT License. The original license is retained in [native/computer-use-macos/LICENSE](native/computer-use-macos/LICENSE) and included in the built helper app.

## Power controls

Closed-lid power controls and recovery behavior in `app/src-tauri/src/lid.rs`, `power.rs` and `setup-lid-mode.sh` adapt Steamcup by Yutai Lin, used under the MIT License. Its [license](docs/third-party/Steamcup-LICENSE) is retained and bundled with the desktop app. Running Steamcup is not required.

## Browser control

`browser/package-lock.json` pins Microsoft's official [Playwright CLI](https://github.com/microsoft/playwright-cli), Playwright and Playwright Core, licensed under Apache-2.0. The browser installer retains the packages' licenses and notices inside the installed runtime. It also copies the selected Node.js executable and its distribution LICENSE into `runtime/licenses/Node-LICENSE`. Node.js and its bundled components retain their original licenses; they are not relicensed under Shifu's MIT license. No browser executable is redistributed by this installer; it uses the user's installed Chrome by default.

## Terminal multiplexer

Shifu.app ships a tmux executable at `Contents/Resources/bin/tmux` (and the download package at `bin/tmux`), built by `scripts/build-tmux.sh` from unmodified release sources. Shifu uses it only when no tmux is installed. These components keep their original licenses and are not relicensed under Shifu's MIT license. Their full license texts are bundled in `Shifu.app/Contents/Resources/licenses/tmux/`.

- [tmux](https://github.com/tmux/tmux) 3.8: ISC License, Copyright (c) Various Authors (per-file copyright lines in its sources). License text: `tmux-COPYING`.
- [libevent](https://github.com/libevent/libevent) 2.1.12-stable, statically linked: 3-clause BSD License, Copyright (c) 2000-2007 Niels Provos, Copyright (c) 2007-2012 Niels Provos and Nick Mathewson, plus the further notices listed in its LICENSE. License text: `libevent-LICENSE`.
- [utf8proc](https://github.com/JuliaStrings/utf8proc) 2.12.0, statically linked: MIT License, Copyright (c) 2014-2021 Steven G. Johnson, Jiahao Chen, Tony Kelman, Jonas Fonseca and other contributors; Copyright (c) 2009, 2013 Public Software Group e. V., Berlin, Germany; its Unicode data files are under the Unicode, Inc. license included in the same file. License text: `utf8proc-LICENSE.md`.
- [jemalloc](https://github.com/jemalloc/jemalloc) 5.4.0, statically linked (tmux uses it on macOS): 2-clause BSD License, Copyright (C) 2002-present Jason Evans, Copyright (C) 2007-2012 Mozilla Foundation, Copyright (C) 2009-present Facebook, Inc. License text: `jemalloc-COPYING`.

The binary otherwise links only macOS system libraries (`/usr/lib/libncurses.5.4.dylib`, `libSystem`, `libresolv`), which are not redistributed.

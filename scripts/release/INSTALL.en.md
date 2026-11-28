# Shifu {{VERSION}} preview · Apple Silicon

The app, CLI and optional Computer Use helper are a matching set. Quit Shifu before replacing it and back up the old app and CLI. Keep existing session and settings directories.

Later upgrades: from this version on Shifu updates itself (Shifu → Check for Updates…, plus a quiet check once a day after opening); the CLI inside the app and an installed Computer Use helper come along. Only a CLI installed separately in `~/.cargo/bin` needs reinstalling from the new package.

1. Run `shasum -a 256 -c SHA256SUMS` next to the downloaded archive; unzip and run it again inside this directory.
2. Use Finder to copy `Shifu.app` into `~/Applications`; create that folder for a first installation.
3. Optional: to use the `shifu` command in your own terminal, install the matching CLI from this directory. The app carries the same CLI inside (`Shifu.app/Contents/Resources/bin/shifu`), so conversation status, notifications and the project manager (项目经理) work without this step:

   ```sh
   mkdir -p "$HOME/.cargo/bin"
   install -m 755 bin/shifu "$HOME/.cargo/bin/shifu"
   rm -rf "$HOME/.cargo/bin/web" && ditto bin/web "$HOME/.cargo/bin/web"
   ```

4. Nothing else to install first: Shifu carries tmux (`Contents/Resources/bin/tmux`, used only when no tmux is installed) and needs no Rust toolchain. When the app opens, its first screen finds the agents and projects on this Mac; a missing agent installs with one click (Install runs its official installer in a Shifu terminal tab, where you see it work; npm-based agents need Node.js first), then tick the projects and press Start. Shifu → Setup Checklist… shows the rest later: Sign In, browser use, Computer Use and notifications, one button each. Permissions default to Agent's own settings; the No confirmation switch on that screen is off. [Remote setup and pairing](docs/REMOTE-RELAY.en.md) is only for connecting another Mac. The package is signed with Shifu's own fixed certificate and not notarized by Apple. From 0.4.1 on every release uses the same certificate, so Accessibility and Screen Recording permissions you granted are kept across upgrades. If blocked by macOS, verify its source and follow the app-specific Privacy & Security prompt.
5. StepFun uses Step Plan subscriptions and additionally requires Node.js 20.6+, isolated Pi 0.73.1 and key setup. See [STEPFUN.md](docs/STEPFUN.md), including desktop-vault compatibility and standalone CLI environment injection. Neither runtime nor keys are bundled.

Optional Computer Use: Shifu → Setup Checklist… → Computer use → Set Up copies the helper Shifu carries into `~/Applications` and opens its permission window (the first `shifu computer` command, or `shifu computer install`, does the same copy). By hand instead: back up an existing `~/Applications/Shifu Computer Use.app`; stop its bridge with the installed CLI's `shifu computer stop` before replacement. In Finder, copy `helpers/Shifu Computer Use.app` into `~/Applications`. Do not launch a development copy from the package. Run `shifu computer doctor`, then `shifu computer permissions` only if needed; set Accessibility and Screen Recording permissions yourself. See the [guide](docs/COMPUTER-USE.md). Skip this helper if unused.

The independent `scripts/autopilot.py` needs Python 3.9+, signed-in Codex and a reviewed run configuration. It follows existing Codex sessions, not StepFun. Installation does not start it: read [AUTOPILOT.md](docs/AUTOPILOT.md). Browser Use sets up its runtime on first use (or `shifu browser install`, or Setup Checklist → Browser use); it needs Node.js 20+, see [BROWSER-USE.md](docs/BROWSER-USE.md). The `shifu web` page pool runtime is in `bin/web` and is installed with the CLI; it also needs Node.js 18+ and a running local Playwright headless Chromium server, see [WEB-POOL.md](docs/WEB-POOL.md).

No accounts, projects, sessions, device identities, server credentials, Pi/Node or browser runtime are included. Native first installation, physical remote Macs and cross-country input still require target-device confirmation. This package does not automatically upgrade a relay or another device.

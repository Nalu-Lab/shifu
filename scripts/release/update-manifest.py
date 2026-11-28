#!/usr/bin/env python3
"""The in-app updater's latest.json: written when a release is packaged, and put on the feed when it is published.

    # package.sh, next to the zip: the manifest for this release's updater package
    python3 scripts/release/update-manifest.py make --tag remote-v0.4.4-preview.1 \\
        --package Shifu-remote-v0.4.4-apple-silicon.app.tar.gz --signature <that>.sig --out latest.json

    # release.yml, before the draft: the manifest names this package and carries its .sig
    python3 scripts/release/update-manifest.py check --tag remote-v0.4.4-preview.1 --manifest latest.json \\
        --package Shifu-remote-v0.4.4-apple-silicon.app.tar.gz

    # update-feed.yml, once the draft is published: the manifest the app reads, with the release's own notes
    python3 scripts/release/update-manifest.py feed --tag remote-v0.4.4-preview.1 --manifest latest.json \\
        --release release.json [--current feed-latest.json] [--force] --out feed.json

The app (tauri.conf.json plugins.updater) reads FEED_URL. `feed` exits 3 without writing when the feed already
offers a newer version than this release (publishing an older draft late must not move users back), unless --force.
Standard library only.
"""
import argparse
import json
import os
import re
import sys
from datetime import datetime, timezone

FEED_TAG = "update-feed-preview"
FEED_URL = f"https://github.com/Nalu-Lab/shifu/releases/download/{FEED_TAG}/latest.json"
PLATFORM = "darwin-aarch64"


def version_of(tag):
    m = re.match(r"remote-v(\d+\.\d+\.\d+)(?:-|$)", tag)
    if not m:
        raise SystemExit(f"tag must look like remote-v1.2.3-...: {tag}")
    return m.group(1)


def key(version):
    return tuple(int(p) for p in version.split("."))


def make(args):
    version = version_of(args.tag)
    repo = os.environ.get("GITHUB_REPOSITORY") or "Nalu-Lab/shifu"
    with open(args.signature, encoding="utf-8") as fh:
        signature = fh.read().strip()
    name = os.path.basename(args.package)
    manifest = {
        "version": version,
        "notes": f"Shifu {version} preview. https://github.com/{repo}/releases/tag/{args.tag}",
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": {PLATFORM: {"url": f"https://github.com/{repo}/releases/download/{args.tag}/{name}", "signature": signature}},
    }
    write(args.out, manifest)


def problems_of(manifest, tag, assets):
    """What makes this release's manifest unfit for the feed."""
    found = []
    version = version_of(tag)
    if manifest.get("version") != version:
        found.append(f"manifest version {manifest.get('version')!r}, release is {version}")
    entry = manifest.get("platforms", {}).get(PLATFORM) or {}
    url, signature = entry.get("url", ""), entry.get("signature", "")
    if not signature:
        found.append(f"no {PLATFORM} signature")
    m = re.match(r"https://github\.com/[\w.-]+/[\w.-]+/releases/download/([^/]+)/([^/]+)$", url)
    if not m or m.group(1) != tag:
        found.append(f"package URL {url!r} is not an asset of {tag}")
    elif m.group(2) not in assets:
        found.append(f"{m.group(2)} is not attached to {tag}")
    return found


def check(args):
    """The release workflow's check of what `make` wrote, before the draft is created."""
    with open(args.manifest, encoding="utf-8") as fh:
        manifest = json.load(fh)
    name = os.path.basename(args.package)
    found = problems_of(manifest, args.tag, {name})
    with open(args.package + ".sig", encoding="utf-8") as fh:
        if manifest.get("platforms", {}).get(PLATFORM, {}).get("signature") != fh.read().strip():
            found.append(f"signature in {args.manifest} is not {name}.sig")
    if found:
        raise SystemExit(f"{args.manifest}:\n  - " + "\n  - ".join(found))
    print(f"{args.manifest} ok: {manifest['version']} → {name}")
    return 0


def feed(args):
    with open(args.manifest, encoding="utf-8") as fh:
        manifest = json.load(fh)
    with open(args.release, encoding="utf-8") as fh:
        release = json.load(fh)  # gh release view --json body,publishedAt,assets,isDraft
    if release.get("isDraft"):
        raise SystemExit(f"{args.tag} is still a draft; the feed only offers published releases")
    assets = {a["name"] for a in release.get("assets", [])}
    found = problems_of(manifest, args.tag, assets)
    if found:
        raise SystemExit(f"{args.tag}: not putting it on the feed:\n  - " + "\n  - ".join(found))
    if args.current and os.path.exists(args.current) and not args.force:
        with open(args.current, encoding="utf-8") as fh:
            current = json.load(fh).get("version", "0.0.0")
        if key(current) > key(manifest["version"]):
            print(f"the feed offers {current}, newer than {manifest['version']}; left as it is (--force to replace)")
            return 3
    body = (release.get("body") or "").strip()
    if body:
        manifest["notes"] = body
    if release.get("publishedAt"):
        manifest["pub_date"] = release["publishedAt"]
    write(args.out, manifest)
    print(f"feed: {manifest['version']} from {args.tag}")
    return 0


def write(path, manifest):
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, ensure_ascii=False, indent=2)
        fh.write("\n")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    m = sub.add_parser("make")
    m.add_argument("--tag", required=True)
    m.add_argument("--package", required=True)
    m.add_argument("--signature", required=True)
    m.add_argument("--out", required=True)
    c = sub.add_parser("check")
    c.add_argument("--tag", required=True)
    c.add_argument("--manifest", required=True)
    c.add_argument("--package", required=True)
    f = sub.add_parser("feed")
    f.add_argument("--tag", required=True)
    f.add_argument("--manifest", required=True)
    f.add_argument("--release", required=True)
    f.add_argument("--current")
    f.add_argument("--force", action="store_true")
    f.add_argument("--out", required=True)
    args = ap.parse_args()
    if args.cmd == "make":
        make(args)
        return 0
    return check(args) if args.cmd == "check" else feed(args)


if __name__ == "__main__":
    sys.exit(main())

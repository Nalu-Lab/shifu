#!/usr/bin/env python3
"""Release consistency check: version files, download links, install steps, site pages and the demo note agree.

    python3 scripts/release/check-docs.py                     # against the release in site/tools/build.py
    python3 scripts/release/check-docs.py --tag remote-v0.4.4-preview.1   # what the release workflow runs

Standard library only. Exits 1 and lists every problem when something disagrees; prints "ok" otherwise.

Release preparation order this check expects: bump the version files, point RELEASE in site/tools/build.py and the
README / docs/INSTALL*.md download links at the new tag, rebuild the site, commit, then push the tag.
"""
import argparse
import importlib.util
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
problems = []


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as fh:
        return fh.read()


def fail(msg):
    problems.append(msg)


def load_build():
    sys.dont_write_bytecode = True  # importing build.py must not leave __pycache__ in site/tools
    spec = importlib.util.spec_from_file_location("site_build", os.path.join(ROOT, "site/tools/build.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def version_of(tag):
    m = re.match(r"remote-v(\d+\.\d+\.\d+)", tag)
    return m.group(1) if m else None


def toml_version(rel):
    m = re.search(r'^\[package\][^\[]*?^version\s*=\s*"([^"]+)"', read(rel), re.M | re.S)
    return m.group(1) if m else None


def lock_versions(rel, names):
    text = read(rel)
    return {n: (re.search(rf'^name = "{re.escape(n)}"\nversion = "([^"]+)"', text, re.M) or [None, None])[1] for n in names}


def check_versions(version):
    found = {
        "Cargo.toml": toml_version("Cargo.toml"),
        "app/src-tauri/Cargo.toml": toml_version("app/src-tauri/Cargo.toml"),
        "remote/Cargo.toml": toml_version("remote/Cargo.toml"),
        "app/package.json": json.loads(read("app/package.json"))["version"],
        "app/src-tauri/tauri.conf.json": json.loads(read("app/src-tauri/tauri.conf.json"))["version"],
    }
    lock = json.loads(read("app/package-lock.json"))
    found["app/package-lock.json"] = lock.get("version")
    found['app/package-lock.json packages[""]'] = lock.get("packages", {}).get("", {}).get("version")
    for rel, names in [("Cargo.lock", ["shifu"]), ("app/src-tauri/Cargo.lock", ["shifu", "shifu-app"]),
                       ("remote/Cargo.lock", ["shifu", "shifu-remote"])]:
        for name, v in lock_versions(rel, names).items():
            found[f"{rel} ({name})"] = v
    for where, v in found.items():
        if v != version:
            fail(f"{where}: version {v}, release is {version}")


LINK_FILES = ["README.md", "README.en.md", "docs/INSTALL.md", "docs/INSTALL.en.md", "site/index.html", "site/en/index.html"]


def check_links(tag):
    for rel in LINK_FILES:
        tags = set(re.findall(r"releases/tag/([\w.\-]+)", read(rel)))
        if not tags:
            fail(f"{rel}: no download link to a release tag")
        for t in sorted(tags - {tag}):
            fail(f"{rel}: download link points to {t}, release is {tag}")


def section(text, start, end):
    a = text.index(start)
    return text[a:text.index(end, a)]


def check_install_texts():
    """README's quick start must be the install guide's, word for word (links differ only by the docs/ prefix)."""
    for readme, guide, head in [("README.md", "docs/INSTALL.md", "### 方式 A"), ("README.en.md", "docs/INSTALL.en.md", "### Option A")]:
        try:
            r = re.sub(r"\]\(docs/", "](", section(read(readme), head, "<details>"))
            g = section(read(guide), head, "<details>")
        except ValueError:
            fail(f"{readme} / {guide}: '{head}' … '<details>' section not found")
            continue
        if r.strip() != g.strip():
            fail(f"{readme}: install section differs from {guide}; copy it from the guide")


def app_bundles_cli(tag, use_tree):
    """Whether the release's Shifu.app carries its own CLI (tauri bundles cli-bundle/ as Resources/bin)."""
    conf = None
    if not use_tree:
        r = subprocess.run(["git", "-C", ROOT, "show", f"{tag}:app/src-tauri/tauri.conf.json"], capture_output=True, text=True)
        if r.returncode == 0:
            conf = r.stdout
        else:
            print(f"note: tag {tag} is not available locally (git fetch --tags); judging the CLI step from the working tree")
    if conf is None:
        conf = read("app/src-tauri/tauri.conf.json")
    resources = json.loads(conf).get("bundle", {}).get("resources", {})
    return any("cli-bundle" in k for k in resources)


def check_cli_step(bundles, version):
    want = "optional" if bundles else "required"
    for rel in ["docs/INSTALL.md", "docs/INSTALL.en.md"]:
        m = re.search(r"<!-- cli-step: (required|optional)\b", read(rel))
        if not m:
            fail(f"{rel}: missing the <!-- cli-step: required|optional --> marker")
        elif m.group(1) != want:
            fail(f"{rel}: says the CLI step is {m.group(1)}, but the {version} app "
                 f"{'carries' if bundles else 'does not carry'} its own CLI; rewrite the install steps, then the marker")
    for rel in ["site/index.html", "site/en/index.html"]:
        m = re.search(r'data-cli-step="(\w+)"', read(rel))
        if not m or m.group(1) != want:
            fail(f"{rel}: install section says the CLI step is {m and m.group(1)}, expected {want}")
    # Package notes ship with what this tree builds, so they follow this tree's tauri.conf.json.
    tree_bundles = app_bundles_cli(None, True)
    for rel in ["scripts/release/INSTALL.en.md", "scripts/release/INSTALL.zh-CN.md"]:
        says_bundled = "Contents/Resources/bin/shifu" in read(rel)
        if says_bundled != tree_bundles:
            fail(f"{rel}: {'mentions' if says_bundled else 'does not mention'} the CLI inside Shifu.app, "
                 f"but this tree {'bundles' if tree_bundles else 'does not bundle'} it")


def check_prompt(build, tag):
    """The Option A prompt (docs/INSTALL*.md, mirrored in README*) must name this release's tag and URL and no other,
    and carry the release's permission sentence: the installing agent should never have to guess which release."""
    if build.RELEASE["tag"] != tag:
        return  # check_site reports the RELEASE mismatch; the prompt is judged against RELEASE
    for lang, c in build.LANGS.items():
        for rel, head in [(c["install"], c["option_a"]), (c["readme"], c["option_a"])]:
            text = read(rel)
            try:
                a = text.index(head)
                prompt = re.search(r"```text\n(.*?)\n```", text[a:], re.S)
            except ValueError:
                prompt = None
            if not prompt:
                fail(f"{rel}: Option A prompt not found")
                continue
            for p in build.prompt_problems(lang, prompt.group(1), rel):
                fail(p)


def check_permission(build, version):
    """The hero's permission line (RELEASE["permission"]) must say what docs/INSTALL*.md says about this release."""
    for lang, c in build.LANGS.items():
        m = re.search(r"<!-- permission-mode[^>]*-->\n(.+?)\n<!-- /permission-mode -->", read(c["install"]), re.S)
        if not m:
            fail(f"{c['install']}: permission-mode block not found")
            continue
        for p in build.permission_problems(lang, m.group(1)):
            fail(p)
    if build.RELEASE["permission"] == "no-confirmation":
        for rel in ["site/index.html", "site/en/index.html"]:
            line = re.search(r'<p[^>]*id="hero-permission"[^>]*>(.*?)</p>', read(rel), re.S)
            if not line or version not in line.group(1):
                fail(f"{rel}: the hero permission line does not name {version}'s no-confirmation default; rebuild the site")


def check_site(build, version, tag):
    if build.RELEASE["version"] != version or build.RELEASE["tag"] != tag:
        fail(f"site/tools/build.py RELEASE is {build.RELEASE['version']} / {build.RELEASE['tag']}, release is {version} / {tag}")
        return
    for lang in build.LANGS:
        out = build.output_path(lang)
        rel = os.path.relpath(out, ROOT)
        try:
            fresh = build.render(lang)
        except (AssertionError, ValueError) as e:
            fail(f"site/tools/build.py cannot render {lang}: {e}")
            continue
        with open(out, encoding="utf-8") as fh:
            if fh.read() != fresh:
                fail(f"{rel}: out of date; run python3 site/tools/build.py and commit the result")
        install = re.search(r'<section[^>]*id="install".*?</section>', fresh, re.S).group(0)
        note = re.search(r'<figcaption id="demo-note">(.*?)</figcaption>', fresh, re.S)
        if not note:
            fail(f"{rel}: demo note (#demo-note) not found")
            continue
        named = set(re.findall(r"\b\d+\.\d+\.\d+\b", note.group(1)))
        demo = build.DEMO["version"]
        if version not in named:
            fail(f"{rel}: demo note does not name the release version {version}")
        for v in sorted(named - {version, demo}):
            fail(f"{rel}: demo note names {v}, which is neither the release ({version}) nor DEMO['version'] ({demo})")
        if demo != version and not re.search(r"当前下载是|current download is", note.group(1)):
            fail(f"{rel}: demo was recorded with {demo} but the note does not say the current download is {version}")
        if install.count("{{") or install.count("}}"):
            fail(f"{rel}: unfilled placeholder in the install section")


def check_updater():
    """The app must read the feed that update-feed.yml writes, and carry the updater's public key."""
    spec = importlib.util.spec_from_file_location("update_manifest", os.path.join(ROOT, "scripts/release/update-manifest.py"))
    manifest = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(manifest)
    updater = json.loads(read("app/src-tauri/tauri.conf.json")).get("plugins", {}).get("updater", {})
    if updater.get("endpoints") != [manifest.FEED_URL]:
        fail(f"app/src-tauri/tauri.conf.json: updater endpoints {updater.get('endpoints')}, expected [{manifest.FEED_URL}]")
    if not updater.get("pubkey"):
        fail("app/src-tauri/tauri.conf.json: no updater pubkey")


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--tag", help="release tag being built (default: RELEASE['tag'] in site/tools/build.py)")
    args = ap.parse_args()
    build = load_build()
    tag = args.tag or build.RELEASE["tag"]
    version = version_of(tag)
    if not version:
        print(f"tag must look like remote-v1.2.3-...: {tag}", file=sys.stderr)
        return 2
    check_versions(version)
    check_links(tag)
    check_install_texts()
    bundles = app_bundles_cli(tag, use_tree=bool(args.tag))
    check_cli_step(bundles, version)
    check_permission(build, version)
    check_prompt(build, tag)
    check_site(build, version, tag)
    check_updater()
    if build.DEMO["version"] != version and not problems:
        print(f"note: the demo video and screenshots are from {build.DEMO['version']}; the note says so (see site/README.md to re-record)")
    if problems:
        print(f"release consistency check failed for {tag}:", file=sys.stderr)
        for p in problems:
            print("  - " + p, file=sys.stderr)
        return 1
    print(f"ok: {tag} (version {version}; app {'carries' if bundles else 'does not carry'} its CLI)")
    return 0


if __name__ == "__main__":
    sys.exit(main())

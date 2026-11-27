#!/usr/bin/env python3
"""Pre-render the two language pages from the bilingual source.

    python3 site/tools/build.py

Reads site/src/index.html and writes site/index.html (Chinese, the default) and site/en/index.html (English).
Standard library only; the output is plain static HTML that is committed, so hosting needs no build step.

Release facts live here and nowhere else: change RELEASE (and DEMO after re-recording), rebuild, commit.
Install steps and the permission-mode note come from docs/INSTALL.md and docs/INSTALL.en.md; the hero's permission
line comes from RELEASE["permission"] and must agree with that note (the build fails otherwise).
scripts/release/check-docs.py checks that the committed pages match a fresh build and the release.
"""
import html
import os
import re
from html.parser import HTMLParser

# ---- The one place to edit for a new release ---------------------------------------------------------------
RELEASE = {
    "version": "0.4.3",                      # shown as "0.4.3 预览版" / "0.4.3 preview"
    "tag": "remote-v0.4.3-preview.1",        # GitHub release tag the download buttons point to
    # How this release starts agents, shown under the hero's download button (see PERMISSION_LINE below):
    #   "no-confirmation": it does not ask and starts agents without approvals (0.4.3);
    #   "asks": it asks once before the first conversation (the releases after 0.4.3).
    # own_label is that release's label for "each agent's own settings" in the new-task form's 权限 menu.
    # check-docs.py fails when docs/INSTALL*.md's permission-mode block says otherwise for this version.
    "permission": "no-confirmation",
    "own_label": "用 Agent 自己的设置",
}
# The build the hero video and screenshots were recorded with. When it differs from RELEASE["version"] the demo
# note says so; after re-recording (site/tools/video/), set it to the release version and the note says "current".
DEMO = {
    "version": "0.4.2",
    "extra_zh": "外加当时源码中的 Gemini CLI",
    "extra_en": "plus Gemini CLI from source at the time",
}
# Canonical, hreflang and og:image are absolute under this address.
SITE_URL = "https://shifu.yutailin.com"
REPO = "https://github.com/Nalu-Lab/shifu"
# -------------------------------------------------------------------------------------------------------------
# Install steps and the permission-mode note are not written here or in src/index.html: they are read from
# docs/INSTALL.md and docs/INSTALL.en.md, so the site and the install guide cannot drift apart.
# scripts/release/check-docs.py fails when the generated pages are stale or the facts disagree.

HERE = os.path.dirname(os.path.abspath(__file__))
SITE = os.path.dirname(HERE)
VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"}

LANGS = {
    "zh": dict(out="index.html", html_lang="zh-CN", root="", other="en", other_lang="en", other_url="en/",
               readme="README.md", privacy_anchor="#隐私", relay_doc="REMOTE-RELAY.md",
               install="docs/INSTALL.md", option_a="### 方式 A", option_b="### 方式 B", copy="复制"),
    "en": dict(out="en/index.html", html_lang="en", root="../", other="zh", other_lang="zh-CN", other_url="../",
               readme="README.en.md", privacy_anchor="#privacy", relay_doc="REMOTE-RELAY.en.md",
               install="docs/INSTALL.en.md", option_a="### Option A", option_b="### Option B", copy="Copy"),
}


class Filter(HTMLParser):
    """Keeps one language: drops elements whose data-l is the other language, turns data-<lang>-<attr> into <attr>."""

    def __init__(self, lang):
        super().__init__(convert_charrefs=False)
        self.lang, self.out, self.skip = lang, [], 0

    def _attrs(self, attrs):
        kept, local = [], {}
        for k, v in attrs:
            m = re.match(r"data-(zh|en)-(.+)", k)
            if m:
                if m.group(1) == self.lang:
                    local[m.group(2)] = v
                continue
            if k == "data-l":
                continue
            kept.append((k, v))
        kept += list(local.items())
        return "".join(f" {k}" if v is None else f' {k}="{html.escape(v, quote=True)}"' for k, v in kept)

    def _drop(self, attrs):
        return any(k == "data-l" and v != self.lang for k, v in attrs)

    def handle_starttag(self, tag, attrs):
        if self.skip:
            if tag not in VOID:
                self.skip += 1
            return
        if self._drop(attrs):
            if tag not in VOID:
                self.skip = 1
            return
        self.out.append(f"<{tag}{self._attrs(attrs)}>")

    def handle_startendtag(self, tag, attrs):
        if self.skip or self._drop(attrs):
            return
        self.out.append(f"<{tag}{self._attrs(attrs)}/>")

    def handle_endtag(self, tag):
        if self.skip:
            self.skip -= 1
            return
        self.out.append(f"</{tag}>")

    def handle_data(self, data):
        if not self.skip:
            self.out.append(data)

    def handle_entityref(self, name):
        if not self.skip:
            self.out.append(f"&{name};")

    def handle_charref(self, name):
        if not self.skip:
            self.out.append(f"&#{name};")

    def handle_comment(self, data):
        if not self.skip:
            self.out.append(f"<!--{data}-->")

    def handle_decl(self, decl):
        self.out.append(f"<!{decl}>")


REPO_ROOT = os.path.dirname(SITE)


def url(path):
    return f"{SITE_URL}/{path}" if SITE_URL else ("./" if path == "" else path)


# The hero's one-line permission summary for each RELEASE["permission"]; {v} is the version, {own} own_label.
PERMISSION_LINE = {
    "no-confirmation": {
        "zh": "{v} 默认用免确认模式启动 Agent；要逐项确认，在新建任务的「权限」里改成「{own}」。",
        "en": "{v} starts agents in no-confirmation mode. To keep approvals, set 权限 to {own} in the new-task form.",
    },
    "asks": {
        "zh": "第一次开对话前，Shifu 会问一次 Agent 的权限，默认选「{own}」；以后在新建任务的「权限」里随时能改。",
        "en": "Before your first conversation, Shifu asks once how agents may act, with {own} (each agent's own settings) "
              "preselected; change it any time under 权限 in the new-task form.",
    },
}
# What the release-specific sentence of the docs' permission-mode block says when the release does not ask yet.
NO_ASK_SENTENCE = {
    "zh": lambda v: rf"{re.escape(v)} 下载包还不会询问[^。]*免确认",
    "en": lambda v: rf"The {re.escape(v)} download does not ask[^.]*?no-confirmation",
}


def permission_problems(lang, block):
    """Where docs/INSTALL*.md's permission-mode block disagrees with RELEASE["permission"]; empty when they agree."""
    v, mode, own = RELEASE["version"], RELEASE["permission"], RELEASE["own_label"]
    doc = LANGS[lang]["install"]
    if mode not in PERMISSION_LINE:
        return [f'RELEASE["permission"] is {mode!r}; expected one of {sorted(PERMISSION_LINE)}']
    says_no_ask = re.search(NO_ASK_SENTENCE[lang](v), block) is not None
    problems = []
    if mode == "no-confirmation" and not says_no_ask:
        problems.append(f"{doc}: the permission-mode block does not say the {v} download starts in no-confirmation mode, "
                        f'but RELEASE["permission"] is "no-confirmation"')
    if mode == "asks" and says_no_ask:
        problems.append(f'{doc}: the permission-mode block says the {v} download does not ask, but RELEASE["permission"] is "asks"')
    if mode == "no-confirmation" and own not in block:
        problems.append(f'{doc}: the permission-mode block does not name RELEASE["own_label"] {own!r}')
    return problems


def hero_permission(lang):
    return html.escape(PERMISSION_LINE[RELEASE["permission"]][lang].format(v=RELEASE["version"], own=RELEASE["own_label"]),
                       quote=False)


# ---- Install steps from docs/INSTALL*.md --------------------------------------------------------------------
def md_inline(text, doc_dir="docs"):
    """The inline Markdown the install guide uses: `code`, **bold**, [text](link). Relative links go to GitHub."""
    out, pos = [], 0
    for m in re.finditer(r"`([^`]+)`", text):
        out.append(_md_spans(text[pos:m.start()], doc_dir))
        out.append(f"<code>{html.escape(m.group(1), quote=False)}</code>")
        pos = m.end()
    out.append(_md_spans(text[pos:], doc_dir))
    return "".join(out)


def _md_spans(text, doc_dir):
    def link(m):
        target = m.group(2)
        if not re.match(r"https?:|#", target):
            path = os.path.normpath(os.path.join(doc_dir, target)).replace(os.sep, "/")
            target = f"{REPO}/blob/main/{path}"
        return f'<a href="{html.escape(target, quote=True)}">{m.group(1)}</a>'
    text = html.escape(text, quote=False)
    text = re.sub(r"\[([^\]]+)\]\(([^)\s]+)\)", link, text)
    return re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", text)


def code_block(body, lang, wrap=False):
    cls = ' class="wrap-text"' if wrap else ""
    return (f'<div class="code" data-collapse><pre{cls}>{html.escape(body, quote=False)}</pre>'
            f'<button class="copy" type="button">{LANGS[lang]["copy"]}</button></div>')


def install_doc(lang):
    """Reads the parts of docs/INSTALL(.en).md the site shows. Raises if the guide's shape changed."""
    c = LANGS[lang]
    doc = open(os.path.join(REPO_ROOT, c["install"]), encoding="utf-8").read()
    perm = re.search(r"<!-- permission-mode[^>]*-->\n(.+?)\n<!-- /permission-mode -->", doc, re.S)
    step = re.search(r"<!-- cli-step: (required|optional)\b", doc)
    a, b = doc.index(c["option_a"]), doc.index(c["option_b"])
    prompt = re.search(r"```text\n(.*?)\n```", doc[a:b], re.S)
    assert perm and step and prompt, f"{c['install']}: permission-mode block, cli-step marker or Option A prompt missing"
    mismatch = permission_problems(lang, perm.group(1))
    assert not mismatch, "; ".join(mismatch)
    # Option B: the numbered list right under its heading; items may hold an indented fenced code block.
    lines = doc[doc.index("\n", b) + 1:].split("\n")
    items, fence = [], None
    for line in lines:
        if fence is not None:
            if line.strip() == "```":
                items[-1].append(("code", "\n".join(fence)))
                fence = None
            else:
                fence.append(line[3:] if line.startswith("   ") else line)
            continue
        m = re.match(r"(\d+)\. (.+)", line)
        if m:
            items.append([("text", m.group(2))])
        elif items and line.startswith("   ```"):
            fence = []
        elif items and line.startswith("   ") and line.strip():
            items[-1].append(("text", line.strip()))
        elif items and line.strip() and not line.startswith("   "):
            break
    assert len(items) >= 3, f"{c['install']}: Option B list not found"
    steps = []
    for item in items:
        parts = [f"<p>{md_inline(v)}</p>" if k == "text" else code_block(v, lang) for k, v in item]
        steps.append("<li>" + "".join(parts) + "</li>")
    return dict(
        prompt=code_block(prompt.group(1), lang, wrap=True),
        steps="<ol class=\"install-steps\">" + "".join(steps) + "</ol>",
        permission=md_inline(perm.group(1).strip()),
        cli_step=step.group(1),
    )


def conditionals(src, flags):
    """<!--if:name-->…<!--/if:name--> is kept when flags[name] is true; <!--ifnot:name--> the other way round."""
    for name, on in flags.items():
        for neg in ("", "not"):
            keep = on != bool(neg)
            pat = rf"<!--if{neg}:{name}-->(.*?)<!--/if{neg}:{name}-->"
            src = re.sub(pat, (lambda m: m.group(1)) if keep else "", src, flags=re.S)
    left = re.findall(r"<!--/?if(?:not)?:\w+-->", src)
    assert not left, f"unknown conditionals: {left}"
    return src


def render(lang):
    """Returns the finished page for one language."""
    c = LANGS[lang]
    src = open(os.path.join(SITE, "src", "index.html"), encoding="utf-8").read()
    src = conditionals(src, {"demo_current": DEMO["version"] == RELEASE["version"]})
    f = Filter(lang)
    f.feed(src)
    f.close()
    page = "".join(f.out)
    self_rel = "" if lang == "zh" else "en/"
    zh_url = url("") if SITE_URL else c["root"] or "./"
    en_url = url("en/") if SITE_URL else ("en/" if lang == "zh" else "./")
    inst = install_doc(lang)
    values = {
        "HTML_LANG": c["html_lang"], "ROOT": c["root"], "L": lang,
        "OTHER_URL": c["other_url"], "OTHER_LANG": c["other_lang"],
        "SELF_URL": url(self_rel) if SITE_URL else "./",
        "ZH_URL": zh_url if lang == "en" else (url("") if SITE_URL else "./"),
        "EN_URL": en_url,
        "VERSION": RELEASE["version"],
        "DEMO_VERSION": DEMO["version"], "DEMO_EXTRA": DEMO["extra_" + lang],
        "OG_IMAGE": url("assets/og.png") if SITE_URL else c["root"] + "assets/og.png",
        "RELEASE_URL": f"{REPO}/releases/tag/{RELEASE['tag']}",
        "ISSUES_URL": f"{REPO}/issues",
        "README": c["readme"], "PRIVACY_ANCHOR": c["privacy_anchor"], "RELAY_DOC": c["relay_doc"],
        "INSTALL_DOC": c["install"],
        "INSTALL_PROMPT": inst["prompt"], "INSTALL_STEPS": inst["steps"],
        "PERMISSION_NOTE": inst["permission"], "CLI_STEP": inst["cli_step"],
        "HERO_PERMISSION": hero_permission(lang),
    }
    page = page.replace("-{L}.", f"-{lang}.")
    for k, v in values.items():
        page = page.replace("{{" + k + "}}", v)
    left = re.findall(r"\{\{[A-Z_]+\}\}|-\{L\}", page)
    assert not left, f"unfilled placeholders: {left}"
    return page


def output_path(lang):
    return os.path.join(SITE, LANGS[lang]["out"])


if __name__ == "__main__":
    for lang in LANGS:
        out = output_path(lang)
        os.makedirs(os.path.dirname(out), exist_ok=True)
        with open(out, "w", encoding="utf-8") as fh:
            fh.write(render(lang))
        print("wrote", os.path.relpath(out, SITE))

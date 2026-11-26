#!/usr/bin/env python3
"""Pre-render the two language pages from the bilingual source.

    python3 site/tools/build.py

Reads site/src/index.html and writes site/index.html (Chinese, the default) and site/en/index.html (English).
Standard library only; the output is plain static HTML that is committed, so hosting needs no build step.

Release facts live here and nowhere else: change RELEASE, rebuild, commit.
"""
import html
import os
import re
from html.parser import HTMLParser

# ---- The one place to edit for a new release ---------------------------------------------------------------
RELEASE = {
    "version": "0.4.3",                      # shown as "0.4.3 预览版" / "0.4.3 preview"
    "tag": "remote-v0.4.3-preview.1",        # GitHub release tag the download buttons point to
    "gemini_in_release": True,               # False: Gemini CLI is badged "下个版本 / next release"
}
# Canonical, hreflang and og:image are absolute under this address.
SITE_URL = "https://shifu.yutailin.com"
# -------------------------------------------------------------------------------------------------------------

HERE = os.path.dirname(os.path.abspath(__file__))
SITE = os.path.dirname(HERE)
VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"}

LANGS = {
    "zh": dict(out="index.html", html_lang="zh-CN", root="", other="en", other_lang="en", other_url="en/",
               readme="README.md", privacy_anchor="#隐私", relay_doc="REMOTE-RELAY.md"),
    "en": dict(out="en/index.html", html_lang="en", root="../", other="zh", other_lang="zh-CN", other_url="../",
               readme="README.en.md", privacy_anchor="#privacy", relay_doc="REMOTE-RELAY.en.md"),
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


def url(path):
    return f"{SITE_URL}/{path}" if SITE_URL else ("./" if path == "" else path)


def render(lang):
    c = LANGS[lang]
    src = open(os.path.join(SITE, "src", "index.html"), encoding="utf-8").read()
    if RELEASE["gemini_in_release"]:
        src = re.sub(r"<!--gemini-next-->.*?<!--/gemini-next-->", "", src, flags=re.S)
    else:
        src = src.replace("<!--gemini-next-->", "").replace("<!--/gemini-next-->", "")
    f = Filter(lang)
    f.feed(src)
    f.close()
    page = "".join(f.out)
    self_rel = "" if lang == "zh" else "en/"
    zh_url = url("") if SITE_URL else c["root"] or "./"
    en_url = url("en/") if SITE_URL else ("en/" if lang == "zh" else "./")
    values = {
        "HTML_LANG": c["html_lang"], "ROOT": c["root"], "L": lang,
        "OTHER_URL": c["other_url"], "OTHER_LANG": c["other_lang"],
        "SELF_URL": url(self_rel) if SITE_URL else "./",
        "ZH_URL": zh_url if lang == "en" else (url("") if SITE_URL else "./"),
        "EN_URL": en_url,
        "VERSION": RELEASE["version"],
        "OG_IMAGE": url("assets/og.png") if SITE_URL else c["root"] + "assets/og.png",
        "RELEASE_URL": f"https://github.com/Nalu-Lab/shifu/releases/tag/{RELEASE['tag']}",
        "README": c["readme"], "PRIVACY_ANCHOR": c["privacy_anchor"], "RELAY_DOC": c["relay_doc"],
    }
    page = page.replace("-{L}.", f"-{lang}.")
    for k, v in values.items():
        page = page.replace("{{" + k + "}}", v)
    left = re.findall(r"\{\{[A-Z_]+\}\}|-\{L\}", page)
    assert not left, f"unfilled placeholders: {left}"
    out = os.path.join(SITE, c["out"])
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, "w", encoding="utf-8") as fh:
        fh.write(page)
    print("wrote", os.path.relpath(out, SITE))


if __name__ == "__main__":
    for lang in LANGS:
        render(lang)

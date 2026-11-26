import { describe, expect, it } from "vitest";
import { cannotShow, pickedPaths, wordFormat } from "./filekind";

describe("file kinds", () => {
  it("previews Word-like documents through textutil with the format their extension names", () => {
    expect(wordFormat("/a/报告.DOCX")).toBe("docx");
    expect(wordFormat("/a/旧版.doc")).toBe("doc");
    expect(wordFormat("/a/笔记.rtf")).toBe("rtf");
    expect(wordFormat("/a/b.odt")).toBe("odt");
    expect(wordFormat("/a/b.wordml")).toBe("wordml");
    for (const p of ["/a/b.pdf", "/a/b.md", "/a/doc", "/a/.rtf", "/a.docx/b"]) expect(wordFormat(p)).toBeNull();
  });

  it("names spreadsheets, slides and iWork files Shifu cannot show", () => {
    for (const p of ["/a/表.xlsx", "/a/B.XLS", "/a/c.pptx", "/a/d.ppt", "/a/e.key", "/a/f.pages", "/a/g.numbers"]) {
      expect(cannotShow(p)).toBe(true);
    }
    for (const p of ["/a/b.docx", "/a/b.pdf", "/a/b.png", "/a/xlsx", "/a/b.txt"]) expect(cannotShow(p)).toBe(false);
  });

  it("turns the Open panel's answer into a list", () => {
    expect(pickedPaths(null)).toEqual([]);
    expect(pickedPaths("/a/b.pdf")).toEqual(["/a/b.pdf"]);
    expect(pickedPaths(["/a/b.pdf", "/a/c.docx"])).toEqual(["/a/b.pdf", "/a/c.docx"]);
  });
});

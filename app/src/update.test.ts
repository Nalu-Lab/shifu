import { afterEach, describe, expect, it } from "vitest";
import { setChinese } from "./i18n";
import { notesFor, progressText } from "./update";

afterEach(() => setChinese(true));

describe("notesFor", () => {
  const body = "下载包包含：\n- 应用内更新\n\n---\n\nThe download includes:\n- In-app updates";
  it("gives each language its half of bilingual release notes", () => {
    expect(notesFor(body, true)).toBe("下载包包含：\n- 应用内更新");
    expect(notesFor(body, false)).toBe("The download includes:\n- In-app updates");
  });
  it("shows notes without a divider as they are", () => {
    expect(notesFor("Only one language", true)).toBe("Only one language");
    expect(notesFor(null, false)).toBe("");
  });
});

describe("progressText", () => {
  it("shows a percentage when the size is known, in the system language", () => {
    setChinese(true);
    expect(progressText({ phase: "download", downloaded: 5242880, total: 20971520 })).toBe("正在下载 25%（5.0 / 20.0 MB）");
    setChinese(false);
    expect(progressText({ phase: "download", downloaded: 1048576, total: null })).toBe("Downloading 1.0 MB");
    expect(progressText({ phase: "restart" })).toBe("Restarting…");
  });
});

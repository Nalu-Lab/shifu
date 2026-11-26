/** Which way a file tab shows a file, decided by its extension alone. */

const extension = (path: string): string => {
  const name = path.slice(path.lastIndexOf("/") + 1);
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
};

/** Word-processor files macOS `textutil` turns into HTML, mapped to its `-format` name. */
const WORD: Record<string, string> = { docx: "docx", doc: "doc", rtf: "rtf", odt: "odt", wordml: "wordml" };

/** The `textutil` format for a Word-like document, or null for anything else. */
export function wordFormat(path: string): string | null {
  return WORD[extension(path)] ?? null;
}

/** Spreadsheets, slides, iWork and archives: Shifu has no viewer, and their bytes are not worth reading. */
const UNSHOWN = new Set(["xlsx", "xlsm", "xls", "ods", "pptx", "ppt", "odp", "key", "pages", "numbers", "zip", "dmg"]);

export function cannotShow(path: string): boolean {
  return UNSHOWN.has(extension(path));
}

export const CANNOT_SHOW = "Shifu 里还看不了这种文件，点「用默认程序打开」。";

/** What the system Open panel returns (nothing, one path, or several) as a list. */
export function pickedPaths(picked: string | string[] | null): string[] {
  return picked === null ? [] : Array.isArray(picked) ? picked : [picked];
}

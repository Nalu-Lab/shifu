/** The first-run screens, the new-task form and the empty page follow the system language, as the menu does
 *  (`menu_in_chinese` in app/src-tauri/src/main.rs): Chinese on a Chinese Mac, English otherwise. The rest of the
 *  page stays Chinese for now. Chinese until the backend answers, and in tests. */
let chinese = true;

export function setChinese(value: boolean): void {
  chinese = value;
}

export const isChinese = (): boolean => chinese;

export const tr = (zh: string, en: string): string => (chinese ? zh : en);

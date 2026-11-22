import { present } from "./newpopover";
import { SIDEBAR_RANGE, TERMINAL_RANGE, getFontSizes, setFontSizes, type FontSizes } from "./fontsize";
import { PRESETS, type Theme, getTheme, resetTheme, sameTheme, setTheme } from "./theme";

const FIELDS: { key: keyof Theme; name: string }[] = [
  { key: "bg", name: "背景" },
  { key: "text", name: "文字" },
  { key: "accent", name: "强调色" },
];

/** Shifu 菜单「外观…」: four presets, the three colors everything else is mixed from, then text sizes. Changes apply
 *  live. */
export function showAppearancePopover(anchor: HTMLElement): void {
  const pop = document.createElement("div");
  pop.className = "popup theme-pop";

  const label = (text: string) => Object.assign(document.createElement("div"), { className: "pop-label", textContent: text });
  const presets = document.createElement("div");
  presets.className = "theme-presets";
  const presetButtons = PRESETS.map((p) => {
    const b = document.createElement("button");
    b.className = "theme-preset";
    const chip = document.createElement("span");
    chip.className = "theme-chip";
    chip.style.background = p.theme.bg;
    chip.style.setProperty("--chip-accent", p.theme.accent);
    b.append(chip, p.name);
    b.addEventListener("click", () => {
      setTheme(p.theme);
      paint();
    });
    presets.append(b);
    return [p.theme, b] as const;
  });

  const inputs = FIELDS.map(({ key, name }) => {
    const row = document.createElement("label");
    row.className = "theme-color";
    const input = document.createElement("input");
    input.type = "color";
    input.addEventListener("input", () => {
      setTheme({ ...getTheme(), [key]: input.value });
      paint();
    });
    row.append(name, input);
    return [key, input, row] as const;
  });

  // − value +, for the terminals (points) and the sidebar (percent).
  const sizeRow = (name: string, key: keyof FontSizes, range: { min: number; max: number; step: number }, unit: string) => {
    const row = document.createElement("div");
    row.className = "theme-color font-size-row";
    const value = document.createElement("span");
    value.className = "font-size-value";
    const step = (sign: -1 | 1, text: string, label: string) => {
      const b = document.createElement("button");
      b.className = "btn font-size-step";
      b.textContent = text;
      b.setAttribute("aria-label", `${name}${label}`);
      b.addEventListener("click", () => {
        setFontSizes({ [key]: getFontSizes()[key] + sign * range.step });
        paint();
      });
      return b;
    };
    const smaller = step(-1, "−", "缩小");
    const bigger = step(1, "+", "放大");
    const controls = document.createElement("span");
    controls.className = "font-size-controls";
    controls.append(smaller, value, bigger);
    row.append(name, controls);
    const update = () => {
      const v = getFontSizes()[key];
      value.textContent = `${v}${unit}`;
      smaller.disabled = v <= range.min;
      bigger.disabled = v >= range.max;
    };
    return { row, update };
  };
  const sizes = [sizeRow("终端", "terminal", TERMINAL_RANGE, ""), sizeRow("侧栏", "sidebar", SIDEBAR_RANGE, "%")];

  const foot = document.createElement("div");
  foot.className = "pop-foot";
  const reset = document.createElement("button");
  reset.className = "btn";
  reset.textContent = "恢复默认";
  reset.addEventListener("click", () => {
    resetTheme();
    paint();
  });
  foot.append(reset);

  const paint = () => {
    const t = getTheme();
    presetButtons.forEach(([theme, b]) => b.setAttribute("aria-pressed", String(sameTheme(theme, t))));
    inputs.forEach(([key, input]) => {
      if (input.value !== t[key]) input.value = t[key];
    });
    sizes.forEach((size) => size.update());
  };
  paint();

  pop.append(
    label("配色"),
    presets,
    label("自定义"),
    ...inputs.map(([, , row]) => row),
    label("字号（终端也可用 ⌘= ⌘- ⌘0）"),
    ...sizes.map((size) => size.row),
    foot,
  );
  // The system color picker takes focus from the window; that must not close the popover.
  present(anchor, "appearance", pop, { suspended: true });
}


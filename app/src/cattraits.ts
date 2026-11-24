export const catOptions = {
  coat: [
    { value: "orange", label: "橘虎斑" }, { value: "calico", label: "三花" },
    { value: "tuxedo", label: "黑白" }, { value: "silver", label: "银灰" },
    { value: "cream", label: "奶油" }, { value: "black", label: "玄黑" },
  ],
  ears: [
    { value: "pointed", label: "尖耳" }, { value: "round", label: "圆耳" },
    { value: "folded", label: "折耳" },
  ],
  tail: [
    { value: "short", label: "短尾" }, { value: "medium", label: "中尾" },
    { value: "long", label: "长尾" },
  ],
  eyes: [
    { value: "amber", label: "琥珀" }, { value: "green", label: "翠绿" },
    { value: "blue", label: "湖蓝" }, { value: "copper", label: "赤铜" },
  ],
} as const;

export type CatAppearance = {
  [K in keyof typeof catOptions]: (typeof catOptions)[K][number]["value"];
};

export const defaultAppearance: CatAppearance = { coat: "orange", ears: "pointed", tail: "long", eyes: "green" };

function sample<T>(items: readonly T[]): T {
  return items[Math.floor(Math.random() * items.length)];
}

export function randomAppearance(previous?: CatAppearance): CatAppearance {
  const coats = catOptions.coat.filter(({ value }) => value !== previous?.coat);
  return {
    coat: sample(coats).value, ears: sample(catOptions.ears).value,
    tail: sample(catOptions.tail).value, eyes: sample(catOptions.eyes).value,
  };
}

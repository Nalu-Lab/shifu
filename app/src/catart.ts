import type { CatAppearance } from "./cattraits";

// Render the same painted cat at two scales. Face crops are shared across tail
// lengths, so changing the tail cannot move the eyes or cheeks in the sidebar.
const portraits = import.meta.glob<string>("./assets/cats/painted/*.webp", {
  eager: true, import: "default", query: "?url&no-inline",
});

export function catPortrait(appearance: CatAppearance, framing: "full" | "face" = "full"): string {
  const { coat, ears, tail, eyes } = appearance;
  const key = framing === "face" ? [coat, ears, eyes, "face"] : [coat, ears, tail, eyes];
  return portraits[`./assets/cats/painted/${key.join("-")}.webp`];
}

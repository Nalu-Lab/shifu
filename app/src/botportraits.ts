import type { ResidentBot } from "./api";
import { catPortrait } from "./catart";
import { defaultAppearance, type CatAppearance } from "./cattraits";
import tabby from "./assets/cats/tabby.png";
import calico from "./assets/cats/calico.png";
import tuxedo from "./assets/cats/tuxedo.png";
import rabbit from "./assets/animals/rabbit.png";
import puppy from "./assets/animals/puppy.png";
import redPanda from "./assets/animals/red-panda.png";
import hedgehog from "./assets/animals/hedgehog.png";

export const botPortraits = { tabby, calico, tuxedo };
const animalPortraits: Record<string, string> = { rabbit, puppy, "red-panda": redPanda, hedgehog };
const originalEyes = import.meta.glob<string>("./assets/cats/original-eyes/*.webp", {
  eager: true, import: "default", query: "?url&no-inline",
});

const presets: Record<keyof typeof botPortraits, CatAppearance> = {
  tabby: defaultAppearance,
  calico: { coat: "calico", ears: "round", tail: "medium", eyes: "amber" },
  tuxedo: { coat: "tuxedo", ears: "pointed", tail: "short", eyes: "amber" },
};

export function residentPortrait(bot: ResidentBot): string {
  const appearance = bot.appearance;
  if (appearance === null) return animalPortraits[bot.id];
  if (bot.id === "tabby" || bot.id === "calico" || bot.id === "tuxedo") {
    const preset = presets[bot.id];
    const { coat, ears, eyes } = appearance;
    if (coat === preset.coat && ears === preset.ears) {
      return eyes === preset.eyes ? botPortraits[bot.id] : originalEyes[`./assets/cats/original-eyes/${bot.id}-${eyes}.webp`];
    }
  }
  return catPortrait(appearance, "face");
}

import { describe, expect, it } from "vitest";
import { DEFAULT_SIZES, readSizes } from "./fontsize";

describe("readSizes", () => {
  it("falls back to the defaults for nothing or nonsense", () => {
    expect(readSizes(null)).toEqual(DEFAULT_SIZES);
    expect(readSizes("not json")).toEqual(DEFAULT_SIZES);
    expect(readSizes('{"terminal":"big","sidebar":null}')).toEqual(DEFAULT_SIZES);
  });

  it("keeps sizes in range and whole", () => {
    expect(readSizes('{"terminal":40,"sidebar":10}')).toEqual({ terminal: 24, sidebar: 80 });
    expect(readSizes('{"terminal":15.6,"sidebar":120}')).toEqual({ terminal: 16, sidebar: 120 });
  });
});

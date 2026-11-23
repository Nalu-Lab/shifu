import { describe, expect, it } from "vitest";
import { PtySize } from "./ptysize";

function backend() {
  const calls: { size: string; done: () => void }[] = [];
  const resize = (cols: number, rows: number) => new Promise<void>((done) => calls.push({ size: `${cols}x${rows}`, done }));
  return { calls, resize };
}
const settle = () => new Promise((r) => setTimeout(r, 0));

describe("PtySize", () => {
  it("ends at the terminal's last size when resizes come quickly", async () => {
    const { calls, resize } = backend();
    const size = new PtySize(resize, () => undefined, 100, 40);
    size.want(100, 41);
    size.want(100, 39);
    size.want(100, 40);
    size.want(90, 40);
    expect(calls.map((c) => c.size)).toEqual(["100x41"]);
    calls[0].done();
    await settle();
    // The sizes in between are skipped; only the latest is sent next.
    expect(calls.map((c) => c.size)).toEqual(["100x41", "90x40"]);
    calls[1].done();
    await settle();
    expect(calls).toHaveLength(2);
  });

  it("catches up a size that changed while the client was attaching", () => {
    const { calls, resize } = backend();
    // Attached at 100x40, but the panel had already grown to 41 rows.
    new PtySize(resize, () => undefined, 100, 40).want(100, 41);
    expect(calls.map((c) => c.size)).toEqual(["100x41"]);
    new PtySize(resize, () => undefined, 100, 41).want(100, 41);
    expect(calls).toHaveLength(1);
  });

  it("moves on after a failed resize", async () => {
    const errors: unknown[] = [];
    const size = new PtySize(() => Promise.reject(new Error("gone")), (e) => errors.push(e), 80, 24);
    size.want(80, 25);
    await settle();
    size.want(80, 26);
    await settle();
    expect(errors).toHaveLength(2);
  });
});

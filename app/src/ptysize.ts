/**
 * Keeps one tmux client's PTY at the terminal's latest size.
 *
 * Each `pty_resize` runs on its own backend thread, so two quick resizes (dragging a split, toggling the sidebar) could
 * land in the wrong order, and a resize while the client was still attaching was dropped. tmux then drew for one size
 * while xterm.js showed another, which leaves stale rows: Codex's "Working" line or its prompt shown twice, one row
 * apart, until the next resize (F25). Here only the latest size is sent, one call at a time.
 */
export class PtySize {
  private sent: string;
  private wanted: [number, number] | null = null;
  private busy = false;

  constructor(
    private readonly resize: (cols: number, rows: number) => Promise<void>,
    private readonly report: (error: unknown) => void,
    /** The size the client was attached at. */
    cols: number,
    rows: number,
  ) {
    this.sent = `${cols}x${rows}`;
  }

  want(cols: number, rows: number): void {
    this.wanted = [cols, rows];
    this.pump();
  }

  private pump(): void {
    if (this.busy || !this.wanted) return;
    const [cols, rows] = this.wanted;
    const key = `${cols}x${rows}`;
    if (key === this.sent) return;
    this.busy = true;
    // Counted as sent even if it fails: a broken client is not retried in a loop; the next resize tries again.
    this.sent = key;
    this.resize(cols, rows)
      .catch(this.report)
      .finally(() => {
        this.busy = false;
        this.pump();
      });
  }
}

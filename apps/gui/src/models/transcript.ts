export interface D1TranscriptRow {
  id: string;
  kind: string;
  content: string;
}

/** GUI-only window and anchor state; runtime facts remain Core-owned. */
export class BoundedTranscript {
  readonly capacity: number;
  rows: D1TranscriptRow[] = [];
  followLatest = true;
  anchor: string | null = null;
  newOutputCount = 0;

  constructor(capacity = 240) {
    this.capacity = Math.max(1, capacity);
  }

  append(row: D1TranscriptRow): void {
    this.rows.push(row);
    if (this.rows.length > this.capacity) {
      this.rows.splice(0, this.rows.length - this.capacity);
    }
    if (this.followLatest) {
      this.anchor = row.id;
      this.newOutputCount = 0;
    } else {
      this.newOutputCount += 1;
    }
  }

  replace(rows: readonly D1TranscriptRow[]): void {
    const known = new Set(this.rows.map((row) => row.id));
    for (const row of rows) {
      if (!known.has(row.id)) {
        this.append(row);
        known.add(row.id);
      }
    }
  }

  reset(rows: readonly D1TranscriptRow[]): void {
    this.rows = [];
    this.followLatest = true;
    this.anchor = null;
    this.newOutputCount = 0;
    this.replace(rows);
  }

  setFollowLatest(followLatest: boolean, anchor?: string): void {
    this.followLatest = followLatest;
    if (followLatest) {
      this.anchor = this.rows.at(-1)?.id ?? null;
      this.newOutputCount = 0;
    } else if (anchor) {
      this.anchor = anchor;
    }
  }

  visible(viewportHeight: number, rowHeight: number): D1TranscriptRow[] {
    const count = Math.min(
      this.rows.length,
      Math.ceil(viewportHeight / Math.max(1, rowHeight)) + 2,
    );
    const anchorIndex = this.followLatest
      ? -1
      : this.rows.findIndex((row) => row.id === this.anchor);
    const end = anchorIndex >= 0 ? anchorIndex + 1 : this.rows.length;
    return this.rows.slice(Math.max(0, end - count), end);
  }
}

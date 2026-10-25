// Pure formatting helpers. No i18n here: copy.ts maps these results to keys.

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const;

/** Decimal units (1 MB = 1,000,000 B), as GNOME shows. 1 decimal below 10, else whole. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '0 B';
  let value = bytes;
  let i = 0;
  while (value >= 1000 && i < UNITS.length - 1) {
    value /= 1000;
    i += 1;
  }
  if (i === 0) return `${Math.round(value)} B`;
  const text = value < 10 ? value.toFixed(1) : String(Math.round(value));
  return `${text.endsWith('.0') ? text.slice(0, -2) : text} ${UNITS[i]}`;
}

/** Downloads at or under this are not worth mentioning (COPY.md: plan > 50 MB). */
export const BIG_DOWNLOAD_BYTES = 50_000_000;

export type EtaBucket =
  | { kind: 'unknown' }
  | { kind: 'seconds' }
  | { kind: 'minute' }
  | { kind: 'minutes'; n: number };

/**
 * ETA seconds to a phrase bucket:
 * null → unknown · ≤45 s → "A few seconds" · ≤90 s → "About a minute" · else "About n minutes".
 */
export function etaBucket(secs: number | null): EtaBucket {
  if (secs === null || !Number.isFinite(secs) || secs < 0) return { kind: 'unknown' };
  if (secs <= 45) return { kind: 'seconds' };
  if (secs <= 90) return { kind: 'minute' };
  return { kind: 'minutes', n: Math.round(secs / 60) };
}

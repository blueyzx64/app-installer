import en from './en.json';

export type Messages = typeof en;
export type MessageKey = keyof Messages;
export type MessageParams = Record<string, string | number>;

/** Look up a message and fill {placeholders}. Unknown keys return the key itself. */
export function t(key: MessageKey, params?: MessageParams): string {
  const raw = (en as Record<string, string>)[key];
  if (raw === undefined) return key;
  if (!params) return raw;
  return raw.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const v = params[name];
    return v === undefined ? whole : String(v);
  });
}

export const messages: Messages = en;

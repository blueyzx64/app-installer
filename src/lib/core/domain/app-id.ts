// Same rule as Rust `AppId::from_name` (crates/domain/src/app.rs): the installed app's id
// is derived from the package's machine name. Keep both in step.
const MAX_SLUG_LEN = 64;
const FALLBACK_SLUG = 'app';

export function appIdFromName(name: string): string {
  let slug = '';
  for (const ch of name) {
    if (/^[A-Za-z0-9]$/.test(ch)) slug += ch.toLowerCase();
    else if (!slug.endsWith('-')) slug += '-';
  }
  slug = slug.slice(0, MAX_SLUG_LEN).replace(/^-+|-+$/g, '');
  return slug === '' ? FALLBACK_SLUG : slug;
}

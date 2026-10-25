// ?scenario=<name> switch for the mock gateways (contracts v1).
export const SCENARIOS = [
  'rpm',
  'deb',
  'appimage',
  'incompatible-arch',
  'already-installed',
  'update',
  'fail-offline',
  'cancel-permission',
  'missing-fuse2',
  'empty-library'
] as const;

export type Scenario = (typeof SCENARIOS)[number];

export const DEFAULT_SCENARIO: Scenario = 'rpm';

export function isScenario(s: string | null | undefined): s is Scenario {
  return !!s && (SCENARIOS as readonly string[]).includes(s);
}

/** Parse `?scenario=` from a query string. null when absent or unknown. */
export function parseScenario(search: string): Scenario | null {
  const v = new URLSearchParams(search).get('scenario');
  return isScenario(v) ? v : null;
}

/** Read the scenario from the page URL (browser only). null elsewhere. */
export function scenarioFromLocation(): Scenario | null {
  const loc = (globalThis as { location?: { search?: string } }).location;
  return loc?.search ? parseScenario(loc.search) : null;
}

/** `?launch=none` / `?file=<path>` dev switches for the mock launch file. */
export function launchOverrideFromLocation(): { none: boolean; file: string | null } {
  const loc = (globalThis as { location?: { search?: string } }).location;
  const q = new URLSearchParams(loc?.search ?? '');
  return { none: q.get('launch') === 'none', file: q.get('file') };
}

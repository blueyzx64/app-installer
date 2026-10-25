// Hand-written from docs/architecture/ipc-contract.md + contracts v1.
// TODO(S5, after S4 regenerates bindings.ts): swap these for re-exports of the generated DTO types.
// Wire shapes: camelCase fields, enums tagged with `kind`.

export type PackageFormat = 'rpm' | 'deb' | 'appImage';
export type InstallScope = 'system' | 'user';

export type Compatibility =
  | { kind: 'ok' }
  | { kind: 'wrongFamily'; madeFor: 'rpmBased' | 'debBased' }
  | { kind: 'wrongArch'; packageArch: string; hostArch: string }
  | { kind: 'alreadyInstalled'; relation: 'same' | 'older' | 'newer'; installedVersion: string }
  | { kind: 'missingRuntime'; what: 'fuse2' }
  | { kind: 'unsupported'; reason: 'appImageType1' | 'dwarfs' | 'unknownFormat' };

export interface PackagePreview {
  /** Server-side preview id, "p<n>". */
  id: string;
  /** Machine name (contracts v1 addition). */
  name: string;
  format: PackageFormat;
  displayName: string;
  version: string;
  summary: string | null;
  /** data: URL (PNG/SVG, <= 256 px) or null. */
  iconDataUrl: string | null;
  sizeBytes: number;
  scope: InstallScope;
  needsPermission: boolean;
  compat: Compatibility;
}

export type InstallPhase =
  | 'preparing'
  | 'waitingForPermission'
  | 'downloading'
  | 'installing'
  | 'finishing';

export type InstallEvent =
  | { kind: 'plan'; extraPackages: number; downloadBytes: number }
  | { kind: 'progress'; phase: InstallPhase; percent: number | null; etaSecs: number | null };

export const ERROR_CODES = [
  'permissionDenied',
  'offline',
  'missingDependencies',
  'conflicts',
  'notEnoughSpace',
  'corruptFile',
  'untrustedSource',
  'backendUnavailable',
  'notFound',
  'busy',
  'unknown'
] as const;

export type ErrorCode = (typeof ERROR_CODES)[number];

export interface CommandError {
  code: ErrorCode;
  detail: string | null;
}

export interface InstalledApp {
  id: string;
  displayName: string;
  version: string;
  iconDataUrl: string | null;
  source: 'appImage' | 'systemPackage';
  /** ISO-8601. */
  installedAt: string;
  canLaunch: boolean;
}

export type InstallOutcome =
  | { kind: 'installed'; app: InstalledApp }
  | { kind: 'cancelled' }
  | { kind: 'failed'; code: ErrorCode; detail: string | null };

export type UninstallOutcome =
  | { kind: 'removed' }
  | { kind: 'cancelled' }
  | { kind: 'failed'; code: ErrorCode; detail: string | null };

export interface DefaultStatus {
  isDefault: boolean;
  askedBefore: boolean;
}

export interface Appearance {
  colorScheme: 'light' | 'dark';
  /** '#RRGGBB' from the portal, null means default blue. */
  accent: string | null;
  highContrast: boolean;
  reducedMotion: boolean;
  controls: { side: 'left' | 'right'; buttons: Array<'close' | 'minimize' | 'maximize'> };
}

/** The `plan` InstallEvent minus its tag. */
export interface Plan {
  extraPackages: number;
  downloadBytes: number;
}

/** Normalise anything a gateway call may reject with into a CommandError. */
export function toCommandError(e: unknown): CommandError {
  if (typeof e === 'object' && e !== null && 'code' in e) {
    const { code, detail } = e as { code: unknown; detail?: unknown };
    if (typeof code === 'string' && (ERROR_CODES as readonly string[]).includes(code)) {
      return { code: code as ErrorCode, detail: typeof detail === 'string' ? detail : null };
    }
  }
  return {
    code: 'unknown',
    detail: e instanceof Error ? e.message : typeof e === 'string' ? e : null
  };
}

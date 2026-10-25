// The ONLY place that maps domain enums to i18n keys. Exhaustive: adding a variant is a compile error
// until copy exists. Returns key + params; callers resolve with t().
import { t, type MessageKey, type MessageParams } from '../../i18n';
import { BIG_DOWNLOAD_BYTES, etaBucket, formatBytes } from '../domain/format';
import type { Compatibility, ErrorCode, InstallPhase, InstalledApp, Plan } from '../domain/types';

export interface Msg {
  key: MessageKey;
  params?: MessageParams;
}

export type ErrorAction = 'close' | 'tryAgain' | null;

function assertNever(x: never): never {
  throw new Error(`Unhandled variant: ${JSON.stringify(x)}`);
}

export function phaseCopy(phase: InstallPhase, ctx: { app: string; plan?: Plan | undefined }): Msg {
  switch (phase) {
    case 'preparing':
      return { key: 'phase.preparing' };
    case 'waitingForPermission':
      return { key: 'perm.waiting' };
    case 'downloading':
      return ctx.plan && ctx.plan.downloadBytes > BIG_DOWNLOAD_BYTES
        ? { key: 'phase.downloadingSize', params: { size: formatBytes(ctx.plan.downloadBytes) } }
        : { key: 'phase.downloading' };
    case 'installing':
      return { key: 'phase.installing', params: { app: ctx.app } };
    case 'finishing':
      return { key: 'phase.finishing' };
    default:
      return assertNever(phase);
  }
}

export function etaCopy(etaSecs: number | null): Msg {
  const b = etaBucket(etaSecs);
  switch (b.kind) {
    case 'unknown':
      return { key: 'eta.unknown' };
    case 'seconds':
      return { key: 'eta.seconds' };
    case 'minute':
      return { key: 'eta.minute' };
    case 'minutes':
      return { key: 'eta.minutes', params: { n: b.n } };
    default:
      return assertNever(b);
  }
}

export interface CompatCopy {
  title: Msg;
  /** null when there is no body line. */
  body: Msg | null;
  /** 'open' = show an Open button (already installed, same version). */
  action: 'open' | null;
}

/** null = installable (ok, or alreadyInstalled.newer which means Update). */
export function compatCopy(compat: Compatibility, app: string): CompatCopy | null {
  const p = { app };
  switch (compat.kind) {
    case 'ok':
      return null;
    case 'wrongFamily':
      return compat.madeFor === 'debBased'
        ? {
            title: { key: 'compat.wrongFamily.debBased.title', params: p },
            body: { key: 'compat.wrongFamily.debBased.body' },
            action: null
          }
        : {
            title: { key: 'compat.wrongFamily.rpmBased.title', params: p },
            body: { key: 'compat.wrongFamily.rpmBased.body' },
            action: null
          };
    case 'wrongArch':
      return {
        title: { key: 'compat.wrongArch.title', params: p },
        body: { key: 'compat.wrongArch.body' },
        action: null
      };
    case 'alreadyInstalled':
      switch (compat.relation) {
        case 'same':
          return {
            title: { key: 'compat.alreadyInstalled.same.title', params: p },
            body: null,
            action: 'open'
          };
        case 'older':
          return {
            title: { key: 'compat.alreadyInstalled.older.title', params: p },
            body: { key: 'compat.alreadyInstalled.older.body' },
            action: null
          };
        case 'newer':
          return null;
        default:
          return assertNever(compat.relation);
      }
    case 'missingRuntime':
      switch (compat.what) {
        case 'fuse2':
          return {
            title: { key: 'compat.missingRuntime.fuse2.title', params: p },
            body: { key: 'compat.missingRuntime.fuse2.body' },
            action: null
          };
        default:
          return assertNever(compat.what);
      }
    case 'unsupported':
      // All unsupported reasons share one plain sentence. Keep the switch exhaustive.
      switch (compat.reason) {
        case 'appImageType1':
        case 'dwarfs':
        case 'unknownFormat':
          return {
            title: { key: 'compat.unsupported.title' },
            body: { key: 'compat.unsupported.body' },
            action: null
          };
        default:
          return assertNever(compat.reason);
      }
    default:
      return assertNever(compat);
  }
}

export interface ErrorCopy {
  message: Msg;
  action: ErrorAction;
}

export function errorCopy(code: ErrorCode, ctx: { app: string; spaceBytes?: number }): ErrorCopy {
  const p = { app: ctx.app };
  const table = {
    permissionDenied: { message: { key: 'error.permissionDenied' }, action: 'close' },
    offline: { message: { key: 'error.offline', params: p }, action: 'tryAgain' },
    missingDependencies: {
      message: { key: 'error.missingDependencies', params: p },
      action: 'close'
    },
    conflicts: { message: { key: 'error.conflicts', params: p }, action: 'close' },
    notEnoughSpace:
      ctx.spaceBytes === undefined
        ? { message: { key: 'error.notEnoughSpaceUnknown' }, action: 'tryAgain' }
        : {
            message: { key: 'error.notEnoughSpace', params: { size: formatBytes(ctx.spaceBytes) } },
            action: 'tryAgain'
          },
    corruptFile: { message: { key: 'error.corruptFile' }, action: 'close' },
    untrustedSource: { message: { key: 'error.untrustedSource', params: p }, action: 'close' },
    backendUnavailable: { message: { key: 'error.backendUnavailable' }, action: 'close' },
    notFound: { message: { key: 'error.notFound', params: p }, action: 'close' },
    busy: { message: { key: 'error.busy', params: p }, action: null },
    unknown: { message: { key: 'error.unknown', params: p }, action: 'tryAgain' }
  } as const satisfies Record<ErrorCode, ErrorCopy>;
  return table[code];
}

/** Toast text when removing an app failed. Plain, no codes. */
export function removeErrorCopy(code: ErrorCode, app: string): Msg {
  switch (code) {
    case 'permissionDenied':
      return { key: 'error.removePermissionDenied', params: { app } };
    case 'notFound':
      return { key: 'error.notFound', params: { app } };
    case 'offline':
    case 'missingDependencies':
    case 'conflicts':
    case 'notEnoughSpace':
    case 'corruptFile':
    case 'untrustedSource':
    case 'backendUnavailable':
    case 'busy':
    case 'unknown':
      return { key: 'error.removeUnknown', params: { app } };
    default:
      return assertNever(code);
  }
}

export interface TrashCopy {
  title: Msg;
  body: Msg;
  confirm: Msg;
  cancel: Msg;
  /** System packages ask for a password, user apps go to the Trash. */
  system: boolean;
}

/** Second line of the alert. null = nothing else goes with the app. Plural: 1 item, N items. */
export function extrasCopy(app: string, alsoRemoves: number): Msg | null {
  if (alsoRemoves <= 0) return null;
  return alsoRemoves === 1
    ? { key: 'trash.body.extras.one', params: { app } }
    : { key: 'trash.body.extras', params: { app, n: alsoRemoves } };
}

export function trashCopy(app: Pick<InstalledApp, 'displayName' | 'source'>): TrashCopy {
  const p = { app: app.displayName };
  switch (app.source) {
    case 'appImage':
      return {
        title: { key: 'trash.title.user', params: p },
        body: { key: 'trash.body.user' },
        confirm: { key: 'trash.confirm.user' },
        cancel: { key: 'trash.cancel' },
        system: false
      };
    case 'systemPackage':
      return {
        title: { key: 'trash.title.system', params: p },
        body: { key: 'trash.body.system' },
        confirm: { key: 'trash.confirm.system' },
        cancel: { key: 'trash.cancel' },
        system: true
      };
    default:
      return assertNever(app.source);
  }
}

/** Resolve a Msg to display text. */
export function text(m: Msg): string {
  return t(m.key, m.params);
}

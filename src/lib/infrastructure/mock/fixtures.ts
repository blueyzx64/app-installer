// Fixtures: a fictional "Lumen Notes" package in rpm / deb / AppImage form, plus a few installed apps.
import type { InstalledApp, PackageFormat, PackagePreview } from '../../core/domain/types';
import type { Scenario } from './scenario';

function svgIcon(bg: string, glyph: string): string {
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">` +
    `<rect width="128" height="128" rx="28" fill="${bg}"/>` +
    `<text x="64" y="86" font-size="64" text-anchor="middle" fill="#fff" font-family="sans-serif">${glyph}</text></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

export const LUMEN_ICON = svgIcon('#f5a524', 'L');
export const LUMEN_NAME = 'lumen-notes';
export const LUMEN_VERSION = '2.4.1';
export const LUMEN_ID = 'lumen-notes';

export const LUMEN_FILES: Record<PackageFormat, string> = {
  rpm: '/home/you/Downloads/lumen-notes-2.4.1.rpm',
  deb: '/home/you/Downloads/lumen-notes_2.4.1.deb',
  appImage: '/home/you/Downloads/Lumen_Notes-2.4.1.AppImage'
};

/** Format implied by a scenario (what the user "opened"). */
export function formatForScenario(s: Scenario): PackageFormat {
  switch (s) {
    case 'deb':
      return 'deb';
    case 'appimage':
    case 'missing-fuse2':
      return 'appImage';
    case 'rpm':
    case 'incompatible-arch':
    case 'already-installed':
    case 'update':
    case 'fail-offline':
    case 'cancel-permission':
    case 'empty-library':
      return 'rpm';
  }
}

/** Guess a scenario from a file name; falls back to the given default. */
export function formatFromPath(path: string): PackageFormat | null {
  const p = path.toLowerCase();
  if (p.endsWith('.rpm')) return 'rpm';
  if (p.endsWith('.deb')) return 'deb';
  if (p.endsWith('.appimage')) return 'appImage';
  return null;
}

export function lumenPreview(
  id: string,
  format: PackageFormat,
  scenario: Scenario
): PackagePreview {
  const user = format === 'appImage';
  const base: PackagePreview = {
    id,
    name: LUMEN_NAME,
    format,
    displayName: 'Lumen Notes',
    version: LUMEN_VERSION,
    summary: 'Quick, calm notes that stay out of your way.',
    iconDataUrl: LUMEN_ICON,
    sizeBytes: user ? 96_000_000 : format === 'deb' ? 48_000_000 : 52_000_000,
    scope: user ? 'user' : 'system',
    needsPermission: !user,
    compat: { kind: 'ok' }
  };
  switch (scenario) {
    case 'incompatible-arch':
      return { ...base, compat: { kind: 'wrongArch', packageArch: 'aarch64', hostArch: 'x86_64' } };
    case 'already-installed':
      return {
        ...base,
        compat: { kind: 'alreadyInstalled', relation: 'same', installedVersion: LUMEN_VERSION }
      };
    case 'update':
      return {
        ...base,
        compat: { kind: 'alreadyInstalled', relation: 'newer', installedVersion: '2.3.0' }
      };
    case 'missing-fuse2':
      return { ...base, compat: { kind: 'missingRuntime', what: 'fuse2' } };
    case 'rpm':
    case 'deb':
    case 'appimage':
    case 'fail-offline':
    case 'cancel-permission':
    case 'empty-library':
      return base;
  }
}

export function lumenApp(source: InstalledApp['source'], version = LUMEN_VERSION): InstalledApp {
  return {
    id: LUMEN_ID,
    displayName: 'Lumen Notes',
    version,
    iconDataUrl: LUMEN_ICON,
    source,
    installedAt: new Date().toISOString(),
    canLaunch: true
  };
}

/** Apps already installed before anything happens. */
export function seedLibrary(scenario: Scenario): InstalledApp[] {
  if (scenario === 'empty-library') return [];
  const apps: InstalledApp[] = [
    {
      id: 'orbit-player',
      displayName: 'Orbit Player',
      version: '1.8.0',
      iconDataUrl: svgIcon('#4b48c9', 'O'),
      source: 'systemPackage',
      installedAt: '2026-09-02T09:30:00Z',
      canLaunch: true
    },
    {
      id: 'pixel-draw',
      displayName: 'Pixel Draw',
      version: '0.9.3',
      iconDataUrl: svgIcon('#1f5fb8', 'P'),
      source: 'appImage',
      installedAt: '2026-10-11T18:05:00Z',
      canLaunch: true
    },
    {
      id: 'tidy-tasks',
      displayName: 'Tidy Tasks',
      version: '3.1.2',
      iconDataUrl: svgIcon('#b0174e', 'T'),
      source: 'appImage',
      installedAt: '2026-10-20T07:45:00Z',
      canLaunch: true
    }
  ];
  if (scenario === 'already-installed') apps.unshift(lumenApp('systemPackage'));
  if (scenario === 'update') apps.unshift(lumenApp('systemPackage', '2.3.0'));
  return apps;
}

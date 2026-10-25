import { describe, expect, it } from 'vitest';
import { createMockGateways } from './index';
import { createMockFilesGateway } from './files-gateway';
import { LUMEN_FILES } from './fixtures';

describe('mock files gateway', () => {
  it('resolves the fixture path for the scenario format', async () => {
    const g = createMockGateways({ scenario: 'deb', launchFile: null });
    expect(await g.files.pickPackage()).toBe(LUMEN_FILES.deb);
    const a = createMockGateways({ scenario: 'appimage', launchFile: null });
    expect(await a.files.pickPackage()).toBe(LUMEN_FILES.appImage);
  });

  it('resolves null when the chooser is cancelled', async () => {
    const f = createMockFilesGateway({ format: 'rpm', cancel: true });
    expect(await f.pickPackage()).toBeNull();
  });
});

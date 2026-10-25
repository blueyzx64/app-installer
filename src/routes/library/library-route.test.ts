// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const goto = vi.hoisted(() => vi.fn());
vi.mock('$app/navigation', () => ({ goto }));
vi.mock('$app/paths', () => ({ resolve: (p: string) => p }));

import TestHost from '$lib/features/library/test-host.svelte';
import { createMockGateways } from '$lib/infrastructure/mock';

beforeEach(() => goto.mockReset());

describe('library route', () => {
  it('empty state: Open a Package… picks a file and opens the installer', async () => {
    const gw = createMockGateways({ scenario: 'empty-library', launchFile: null });
    const pick = vi.spyOn(gw.files, 'pickPackage').mockResolvedValue('/tmp/x.rpm');
    render(TestHost, { gateways: gw });
    const btn = await screen.findAllByRole('button', { name: 'Open a Package…' });
    await fireEvent.click(btn[btn.length - 1]!);
    await waitFor(() => expect(goto).toHaveBeenCalledWith('/install'));
    expect(pick).toHaveBeenCalledOnce();
  });

  it('a dismissed chooser does nothing', async () => {
    const gw = createMockGateways({ scenario: 'empty-library', launchFile: null });
    const pick = vi.spyOn(gw.files, 'pickPackage').mockResolvedValue(null);
    render(TestHost, { gateways: gw });
    const btn = await screen.findAllByRole('button', { name: 'Open a Package…' });
    await fireEvent.click(btn[0]!);
    await waitFor(() => expect(pick).toHaveBeenCalled());
    expect(goto).not.toHaveBeenCalled();
  });
});

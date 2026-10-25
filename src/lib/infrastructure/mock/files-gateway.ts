import type { PackageFormat } from '../../core/domain/types';
import type { FilesGateway } from '../../core/ports/files-gateway';
import { LUMEN_FILES } from './fixtures';

/** `?pick=cancel` makes the fake chooser behave as if dismissed. */
export function pickCancelledFromLocation(): boolean {
  const loc = (globalThis as { location?: { search?: string } }).location;
  return new URLSearchParams(loc?.search ?? '').get('pick') === 'cancel';
}

export function createMockFilesGateway(opts: {
  format: PackageFormat;
  cancel?: boolean;
}): FilesGateway {
  return {
    async pickPackage() {
      const cancel = opts.cancel ?? pickCancelledFromLocation();
      return cancel ? null : LUMEN_FILES[opts.format];
    }
  };
}

import type {
  Appearance,
  DefaultStatus,
  InstallEvent,
  InstallOutcome,
  PackagePreview
} from '../domain/types';
import type { LibraryGateway } from './library-gateway';

export type { LibraryGateway, UninstallPlan } from './library-gateway'; // lane S12

export interface InstallerGateway {
  takeLaunchFile(): Promise<string | null>;
  openPackage(path: string): Promise<PackagePreview>;
  install(previewId: string, onEvent: (e: InstallEvent) => void): Promise<InstallOutcome>;
  cancelInstall(previewId: string): Promise<void>;
  /** Returns an unsubscribe function. */
  onOpenFile(cb: (path: string) => void): () => void;
}

export interface DefaultsGateway {
  status(): Promise<DefaultStatus>;
  makeDefault(): Promise<void>;
  dismissPrompt(): Promise<void>;
}

export interface AppearanceGateway {
  get(): Promise<Appearance>;
  /** Returns an unsubscribe function. */
  onChange(cb: (a: Appearance) => void): () => void;
}

export interface WindowGateway {
  close(): Promise<void>;
  minimize(): Promise<void>;
  startDragging(): Promise<void>;
}

import type { FilesGateway } from './files-gateway'; // lane S4b
export type { FilesGateway } from './files-gateway'; // lane S4b

export interface Gateways {
  installer: InstallerGateway;
  library: LibraryGateway;
  defaults: DefaultsGateway;
  appearance: AppearanceGateway;
  window: WindowGateway;
  files: FilesGateway; // lane S4b
}

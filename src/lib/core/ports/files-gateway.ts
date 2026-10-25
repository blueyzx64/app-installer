// F-07: the "Open a Package…" chooser. Drops (F-06) arrive via InstallerGateway.onOpenFile.
export interface FilesGateway {
  /** Opens the system file chooser. Resolves the canonical path, or null if dismissed. */
  pickPackage(): Promise<string | null>;
}

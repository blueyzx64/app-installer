# Copy and Microcopy

Voice: calm, short and specific, like a helpful friend. Use sentence case for text and title case for buttons (macOS convention). Use the app's display name, never a file or package name. Use "your computer", not "the system".

**Banned words in the UI:** RPM, DEB, AppImage (except in the "Details" disclosure), package manager, dnf, apt, PackageKit, polkit, root, sudo, dependency/dependencies, repository, transaction, path, terminal, MIME, binary.

All strings live in `src/lib/i18n/en.json`. `core/copy/copy.ts` maps domain enums to keys. The keys below are the contract.

## Installer view

| Key | Text |
|---|---|
| `dmg.hint` | Drag {app} to Applications |
| `dmg.folder` | Applications |
| `dmg.folderUpdate` | Update |
| `dmg.needsPassword` | Needs your password |
| `dmg.keyboardHint` | Press Enter to install |
| `perm.waiting` | Waiting for permission… |
| `perm.sub` | Enter your password in the window that appeared. |

## Progress phases (`InstallPhase` → text)

| Phase | Key | Text |
|---|---|---|
| `preparing` | `phase.preparing` | Getting ready… |
| `waitingForPermission` | `perm.waiting` | Waiting for permission… |
| `downloading` | `phase.downloading` | Getting a few things it needs |
| `downloading` + plan > 50 MB | `phase.downloadingSize` | Getting a few things it needs ({size}) |
| `installing` | `phase.installing` | Installing {app}… |
| `finishing` | `phase.finishing` | Finishing up… |
| ETA | `eta.seconds` / `eta.minute` / `eta.minutes` / `eta.unknown` | A few seconds left · About a minute left · About {n} minutes left · (empty) |

## Done

| Key | Text |
|---|---|
| `done.title` | {app} is installed |
| `done.body` | You can find it with your other apps. |
| `done.open` / `done.done` | Open · Done |

## Compatibility

| Kind | Title | Body |
|---|---|---|
| `wrongFamily.debBased` | {app} can't be installed here | This version was made for Ubuntu-based computers. Check the developer's website for a version for Fedora. |
| `wrongFamily.rpmBased` | {app} can't be installed here | This version was made for Fedora-based computers. Check the developer's website for a version for Ubuntu. |
| `wrongArch` | {app} can't be installed here | This version was made for a different kind of processor. |
| `alreadyInstalled.same` | {app} is already installed | (button **Open**) |
| `alreadyInstalled.older` | A newer version of {app} is already installed | You don't need to do anything. |
| `missingRuntime.fuse2` | {app} needs something your computer doesn't have | Ask for help installing "FUSE 2 support", or get a different version of this app. |
| `unsupported` | This file can't be opened | It may be an older or unusual kind of app file. |

## Errors (`ErrorCode` → text + action)

| Code | Message | Action |
|---|---|---|
| `permissionDenied` | You don't have permission to install apps on this computer. | Close |
| `offline` | {app} needs an internet connection to finish installing. | Try Again |
| `missingDependencies` | Some things {app} needs aren't available. | Close |
| `conflicts` | {app} can't be installed alongside another app on this computer. | Close |
| `notEnoughSpace` | There isn't enough space. Free up {size} and try again. | Try Again |
| `corruptFile` | This file is damaged or isn't an app. Try downloading it again. | Close |
| `untrustedSource` | {app} isn't from a known developer. | Close (later: Install Anyway) |
| `backendUnavailable` | Installing apps isn't available right now. Restart your computer and try again. | Close |
| `busy` | Another app is being installed. {app} will start when it's done. | (none) |
| `unknown` | Something went wrong while installing {app}. | Try Again |

Every error view has a collapsed **Details** disclosure with the technical code, the backend message and the log path. That is the only place technical text appears.

## Default handler sheet

| Key | Text |
|---|---|
| `default.title` | Open app files with Installer? |
| `default.body` | Downloaded apps will open here, ready to drag into Applications. |
| `default.make` / `default.notNow` | Make Default · Not Now |
| `default.toast` | Installer will now open app files |

## Library and uninstall

| Key | Text |
|---|---|
| `library.title` | Applications |
| `library.empty` | Apps you install will appear here. |
| `library.open` | Open a Package… |
| `trash.title.user` | Move "{app}" to the Trash? |
| `trash.body.user` | You can restore it from the Trash later. |
| `trash.title.system` | Remove "{app}"? |
| `trash.body.system` | You'll be asked for your password. |
| `trash.body.extras` | This also removes {n} other items that only {app} used. |
| `trash.confirm.user` / `trash.confirm.system` / `trash.cancel` | Move to Trash · Remove · Cancel |

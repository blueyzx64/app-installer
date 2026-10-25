# Folder Structure

This page shows the layout as built on the integration branch, then what is still planned. Items under "Planned" do not exist yet. Create files in the places shown and nowhere else. If you need a new top-level folder, add an ADR first.

## As built ✅

```
app-installer/
├─ CLAUDE.md  README.md  start-workflow.md
├─ .editorconfig  .gitignore  .prettierrc  .prettierignore
├─ .claude/skills/                   task-specific guides for Claude Code
├─ .github/
│  ├─ pull_request_template.md
│  └─ workflows/{ci.yml,release.yml}
├─ docs/                             architecture, adr, product, design, platform, DEVELOPMENT.md, handoff (historical)
├─ design/prototype/                 interactive HTML prototype
├─ scripts/                          Node helpers run by `pnpm lint:copy` and `pnpm test`
│  ├─ lint-copy.mjs  lint-copy.allow.json  lint-copy.test.mjs   banned-word check over en.json
│  └─ eslint-boundaries.test.mjs     checks the TS dependency rule
├─ package.json  pnpm-lock.yaml  svelte.config.js  vite.config.ts  tsconfig.json  eslint.config.js
│
├─ src/                              FRONTEND (SvelteKit SPA)
│  ├─ app.html  app.d.ts
│  ├─ routes/{+layout.ts,+layout.svelte,+page.svelte,install/[[previewId]]/+page.svelte,library/+page.svelte}
│  └─ lib/
│     ├─ app/{context.ts,env.ts}
│     ├─ core/
│     │  ├─ domain/{types,installer-machine,library-machine,default-prompt-machine,format}.ts (+ tests)
│     │  ├─ ports/index.ts           all gateway interfaces live in this one file for now
│     │  └─ copy/copy.ts
│     ├─ infrastructure/
│     │  ├─ tauri/{bindings.ts (generated),invoke.ts,index.ts,installer-gateway.ts,library-gateway.ts,defaults-gateway.ts,appearance-gateway.ts,window-gateway.ts}
│     │  └─ mock/{index.ts,fixtures.ts,scenario.ts,install-script.ts}
│     ├─ features/
│     │  ├─ installer/              controller, DmgView, DraggableAppIcon, ApplicationsFolder, PermissionShade, ProgressView, DoneView, IncompatibleView, use-drag
│     │  ├─ library/LibraryController.svelte.ts
│     │  └─ default-handler/DefaultHandlerController.svelte.ts
│     ├─ ui/                        Alert, AppIcon, Button, Disclosure, ProgressBar, Sheet, Spinner, Toast, icons/, focus.ts, motion.ts, tokens.css, base.css
│     ├─ i18n/{en.json,index.ts}
│     └─ assets/fonts/              bundled Inter
│
└─ src-tauri/                        BACKEND (Cargo workspace root)
   ├─ Cargo.toml  Cargo.lock  build.rs  tauri.conf.json
   ├─ capabilities/main.json
   ├─ icons/  linux/{installer.desktop,io.github.blueyzx64.Installer.metainfo.xml}
   ├─ src/                           interface adapters + composition root
   │  ├─ main.rs lib.rs composition.rs state.rs cli.rs dto.rs error.rs events.rs bindings.rs
   │  ├─ bin/export-bindings.rs      used by `pnpm bindings`
   │  └─ commands/{mod,package,install,library,defaults,appearance}.rs
   └─ crates/
      ├─ domain/src/{lib,package,version,compat,host,install,app,appearance,preview,error}.rs
      ├─ application/src/
      │  ├─ lib.rs  registry.rs      preview registry (was planned as preview_registry.rs)
      │  ├─ ports/{packages,host,desktop,ledger,files,appearance}.rs
      │  ├─ use_cases/{open_package,install_package,cancel_install,list_installed,uninstall_app,launch_app,default_handler,appearance}.rs
      │  └─ testing/mod.rs           fakes for every port
      └─ infrastructure/src/{lib.rs,inspect/mod.rs}
```

Differences from the original plan:

- Ports are fewer, larger files. `packages.rs` holds `SystemPackages`, `UserApps` and the inspector. `files.rs` holds the `FileProbe` port. `desktop.rs` also covers defaults and launching.
- The preview registry is `registry.rs`, not `preview_registry.rs`.
- Domain has no `format.rs`. The package format lives in `package.rs`. `preview.rs` is new.
- All TS gateway interfaces are in `core/ports/index.ts`. The plan has one file per gateway. Contracts v2 adds `files-gateway.ts` as its own file.
- `scripts/` is new. It holds the copy linter and the dependency-rule test.
- Mock gateways are assembled in `mock/index.ts` with `scenario.ts` and `install-script.ts`, not as separate `*-gateway.ts` files.

## Planned (expansion lanes, contracts v2) and not in the tree yet

| Path | Purpose | Feature |
|---|---|---|
| `src-tauri/src/logging.rs` | log to stderr and `$XDG_STATE_HOME/<app-id>/log/` | F-60 |
| `src-tauri/src/dropzone.rs` | listens to the window drag-drop event and emits `app://open-file` | F-06 |
| `src-tauri/src/commands/files.rs` | `pick_package` command (file chooser) | F-07 |
| `src/lib/core/ports/files-gateway.ts`, `infrastructure/{tauri,mock}/files-gateway.ts` | `FilesGateway.pickPackage()` | F-07 |
| `application/src/use_cases/uninstall_plan.rs` | `UninstallPlan` use case, `SystemPackages::removal_plan` | F-42 |
| `infrastructure/src/file_probe.rs` | real `FileProbe` | F-03 |
| `infrastructure/src/host/` | real `HostProbe` (os-release, arch, libfuse2) | |
| `infrastructure/src/{packagekit,dnf5daemon}/` | system package backends | F-23, F-24, F-27, F-28, F-30, F-31, F-42 |
| `infrastructure/src/inspect/{rpm,deb,appimage,elf}.rs` | package readers (never execute, ADR-0005) | F-04, F-05, F-08, F-09 |
| `infrastructure/src/{appimage_store,desktop_entries,defaults,ledger,launcher,appearance}.rs` | user-scope install and desktop integration | |
| `src/lib/features/library/{AppGrid,AppTile,TrashConfirmAlert}.svelte` | library views | F-40 to F-43 |
| `src/lib/features/default-handler/DefaultPromptSheet.svelte` | first-launch prompt | F-50 |
| `src/lib/features/appearance/`, `ui/Window{Chrome,Controls}.svelte` | GNOME appearance and window controls | |
| `capabilities/library.json`, `.github/workflows/e2e-nightly.yml`, `e2e/`, `playwright.config.ts` | end-to-end tests | M5 |

## Placement rules (quick reference)

| You are adding… | Put it in… |
|---|---|
| A business rule ("is this .rpm OK on this host?") | `src-tauri/crates/domain` |
| A new operation the UI can trigger | use case in `crates/application/use_cases` → command in `src-tauri/src/commands` → gateway method in `src/lib/core/ports` + both `infrastructure/tauri` and `infrastructure/mock` |
| Talking to a new system service | a port trait in `application/ports` + an adapter in `crates/infrastructure` |
| A user-visible sentence | `src/lib/i18n/en.json`, selected through `core/copy` |
| A reusable visual element | `src/lib/ui` (no feature knowledge) |
| A screen or flow piece | `src/lib/features/<feature>` |
| A Tauri permission | `src-tauri/capabilities/*.json`, as narrow as possible, with a comment explaining why |

## Skills

| Skill | Use when |
|---|---|
| [`apple-hig-design`](../../.claude/skills/apple-hig-design/SKILL.md) | Building or changing any visual component, motion, or layout |
| [`svelte5-frontend`](../../.claude/skills/svelte5-frontend/SKILL.md) | Writing Svelte/TS: runes, controllers, gateways, routing |
| [`tauri-ipc`](../../.claude/skills/tauri-ipc/SKILL.md) | Adding or changing a command, event, channel or capability |
| [`rust-clean-architecture`](../../.claude/skills/rust-clean-architecture/SKILL.md) | Any Rust in `src-tauri` |
| [`linux-package-backends`](../../.claude/skills/linux-package-backends/SKILL.md) | PackageKit, dnf5daemon, AppImage, .desktop, MIME, polkit |
| [`gnome-integration`](../../.claude/skills/gnome-integration/SKILL.md) | Appearance, button layout, accent, Wayland window behaviour |
| [`plain-language-copy`](../../.claude/skills/plain-language-copy/SKILL.md) | Any user-facing text or error message |
| [`release-packaging`](../../.claude/skills/release-packaging/SKILL.md) | tauri.conf bundle config, RPM, CI, signing |

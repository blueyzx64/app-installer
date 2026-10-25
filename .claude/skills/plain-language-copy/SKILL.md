---
name: plain-language-copy
description: Write or review any user-facing text — labels, progress phrases, errors, alerts, toasts — for non-technical users. Use whenever a string is added or changed in src/lib/i18n or a component.
---

# Plain-language copy

The source of truth is `docs/design/COPY.md`. Add new keys there and in `src/lib/i18n/en.json` together.

## Rules
1. Name the **app** ("Lumen Notes"), never the file (`lumen-notes_2.4.1.rpm`).
2. Say what happens and what to do next, in one or two short sentences. No blame ("You entered…"), no codes.
3. Banned words in the UI: RPM, DEB, AppImage, package manager, dnf, apt, PackageKit, polkit, root, sudo, dependency, repository, transaction, path, terminal, MIME, binary. These may appear only inside the collapsed **Details** disclosure.
4. Buttons are verbs in title case: **Open**, **Done**, **Try Again**, **Move to Trash**, **Make Default**, **Not Now**. The destructive button names the action, never "OK".
5. Time: "A few seconds left", "About a minute left", "About 3 minutes left". No percentages in text (the bar shows them).
6. Cancelling the password prompt is not an error. Show no message at all.
7. Use the ellipsis `…` (U+2026) for ongoing actions and for buttons that open another window ("Open a Package…").
8. Write in sentence case. Use curly quotes around app names in alerts: “Lumen Notes”.

## Mapping code → copy
`src/lib/core/copy/copy.ts` is the only place that maps `InstallPhase`, `ErrorCode` and `Compatibility` to i18n keys. When you add an enum variant, TypeScript exhaustiveness (`satisfies Record<ErrorCode, …>`) forces you to add copy.

## Review checklist
- [ ] No banned words (CI runs `pnpm lint:copy`, a grep over en.json)
- [ ] Each error has an action
- [ ] It reads well when read aloud to someone who has never used Linux

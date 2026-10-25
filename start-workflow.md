# Start Workflow: Agent Swarm Build Plan

How one **orchestrator** runs up to **10 Sonnet** and **50 Haiku** agents at a time to build every feature in [`docs/product/FEATURES.md`](docs/product/FEATURES.md) to done, wave by wave, following [`docs/product/ROADMAP.md`](docs/product/ROADMAP.md).

> Who reads what:
> - **Orchestrator** reads all of this file.
> - **Sonnet leads** read §1–§7 and their own role card in §4.
> - **Haiku workers** read only their ticket, plus §5.3 and §6.

---

## 1. Roles and hard limits

| Tier | Who | How many at once | Job | Never |
|---|---|---|---|---|
| **Orchestrator (O)** | Main session | 1 | Plans waves, freezes contracts, answers questions, merges lanes, owns shared memory, pushes | Writes feature code itself (except trivial merge fixes) |
| **Lead** | `model: "sonnet"` | **≤ 10** | Owns one lane: designs, writes the hard code, splits work into Haiku tickets, integrates and verifies | Edits files outside its lane, writes shared memory |
| **Worker** | `model: "haiku"` | **≤ 50** | Does exactly one ticket: one small, fully specified change plus its test | Makes design choices, touches files not named in its ticket, spawns agents |

Spawn every agent with the Agent tool: `model: "sonnet"` or `model: "haiku"`, `isolation: "worktree"`, `run_in_background: true`.
If the user has opted into workflows (for example by saying "run the workflow"), O may drive a wave with the Workflow tool instead, keeping the same limits.

## 2. Token-minimisation rules (everyone)

1. **Caveman for prose.** Every agent loads the `caveman:caveman` skill first. All agent-to-agent text uses caveman style: reports, memory files, ticket prose, questions. **Exceptions, written normally:** code, code comments, docs in `docs/`, UI strings in `en.json`, commit messages and ADRs.
   *If the skill isn't available:* short sentences, small words, no filler, at most 3 bullets, and code stays exact.
2. **Skill budget: caveman plus at most 3 project skills** per lead, and caveman plus **exactly 1** per Haiku ticket. The skills for each role are listed in §4. Don't load others.
3. **Read narrowly.** Read only the doc sections your context pack names. Use Grep, or Read with `offset`/`limit`, rather than whole files. Don't re-read a file you just edited.
4. **Hand off by pointer.** Pass the path, the commit SHA and one line of description. Never paste file contents into reports or memory.
5. **Test narrowly.** Workers run only their ticket's test command (`cargo test -p <crate> <filter>`, `pnpm vitest run <file>`). Leads run their lane's suite. Only O runs the full suite, at the gate.
6. **Fail fast.** After 2 failed attempts at the same error, write `STATUS: blocked` and the reason, then stop. Don't loop.
7. **Final report: 120 words or fewer**, using the template in §6.3.
8. **Keep memory small.** Your own memory file stays at 60 lines or fewer. Replace the `NOW` block; don't append history to it.

## 3. Memory system

Memory lives **outside the repo**, so every worktree sees the same files:

```
SWARM=/home/user/.swarm/app-installer
$SWARM/
├─ shared/                 ← ONLY the orchestrator writes here (others: read-only)
│  ├─ brief.md             30-line project brief + links (the base context pack)
│  ├─ board.md             wave, lanes, tickets → status, owner, SHA
│  ├─ contracts.md         FROZEN signatures: port traits, DTO names, gateway methods, event names
│  ├─ decisions.md         answers to questions: D-001, D-002 … (append-only)
│  └─ ownership.md         path glob → owning lead
└─ agents/                 ← each agent writes ONLY its own file or folder
   ├─ S2.md                lead memory
   ├─ S2/tickets/T-B-07.md tickets written by that lead (read by its workers)
   └─ H-B-07.md            worker memory (one per ticket)
```

**Enforcement.** O runs `chmod 444 $SWARM/shared/*.md` after every write, and `chmod u+w` before the next one. Every agent prompt says: "Don't write to `shared/`, or to another agent's file." All agents share one Unix user, so these permissions are a guard against mistakes, not real security.

**Communication protocol (no direct chatter):**
- Agent → O: write `ASK O: <question>` in your own memory file and keep working on whatever isn't blocked.
- O → everyone: an answer goes into `shared/decisions.md` as `D-0xx`, and contract changes go into `shared/contracts.md`. O tells affected agents with SendMessage: "see D-0xx".
- Lead → worker: the ticket file in `agents/<lead>/tickets/`.
- Worker → lead: `agents/H-…md` (status, SHA, notes).
- O polls cheaply: `grep -H '^STATUS\|^ASK' $SWARM/agents/*.md`

**Own memory file format** (line 2 is always STATUS):

```
# S2 · lane B · PackageKit
STATUS: working            # working | blocked | review | done
NOW: T-B-03 transaction progress stream
DONE: T-B-01 a1b2c3d, T-B-02 d4e5f6a
ASK O: PK Status enum 13 = what? need pk-enum.h ver
NOTE: zbus proxy in packagekit/proxy.rs, reuse
```

## 4. Lanes and role cards

Lane = one lead, its workers, and the paths it owns. Shared files have a single owner. Everyone else asks for changes through `ASK O`, and O routes them to the owner.

| Lane | Lead | Owns (paths) | Skills (+ caveman) | Feature IDs |
|---|---|---|---|---|
| **A · Rust core** | S1 | `src-tauri/crates/domain/**`, `crates/application/**`, `src-tauri/Cargo.toml` [workspace] | `rust-clean-architecture` | domain rules for F-03, F-10–F-15, F-54 parse; all use cases |
| **B · System packages** | S2 | `crates/infrastructure/src/{packagekit,dnf5daemon,inspect/rpm.rs,inspect/deb.rs,host}/**` | `rust-clean-architecture`, `linux-package-backends` | F-05, F-23, F-24, F-27, F-28, F-31, F-42 (backend), F-08, F-09, F-30 |
| **C · AppImage + desktop** | S3 | `crates/infrastructure/src/{inspect/appimage.rs,inspect/elf.rs,appimage_store.rs,desktop_entries.rs,defaults.rs,ledger.rs,launcher.rs}`, `infrastructure/tests/fixtures/**` | `rust-clean-architecture`, `linux-package-backends` | F-04, F-15, F-22, F-29 (launch), F-41, F-43, F-50 (backend) |
| **D · Tauri shell + IPC** | S4 | `src-tauri/src/**`, `src-tauri/capabilities/**`, `src-tauri/tauri.conf.json` (`app`/`windows`), **`src/lib/infrastructure/tauri/bindings.ts`** (sole regenerator) | `tauri-ipc`, `rust-clean-architecture` | F-01, F-02, F-06, F-07, all commands |
| **E · Frontend core** | S5 | `src/lib/core/**`, `src/lib/app/**`, `src/lib/infrastructure/**` (except bindings.ts), `src/lib/features/*/*Controller.svelte.ts`, `src/routes/**`, **`src/lib/i18n/en.json`** | `svelte5-frontend`, `tauri-ipc`, `plain-language-copy` | state machines, gateways, mocks (F-61), copy (F-25, F-26) |
| **F · UI / design system** | S6 | `src/lib/ui/**` (incl. **`tokens.css`**), `src/lib/features/**/*.svelte`, `src/lib/assets/**` | `apple-hig-design`, `svelte5-frontend` | F-20, F-21, F-29 (view), F-40 (view), all views |
| **G · GNOME integration** | S7 | `crates/infrastructure/src/appearance.rs`, `src/lib/features/appearance/**`, `src/lib/ui/Window{Chrome,Controls}.svelte` | `gnome-integration`, `apple-hig-design`, `rust-clean-architecture` | F-53, F-54, F-55 |
| **H · Release + CI** | S8 | `.github/**`, `src-tauri/linux/**`, `tauri.conf.json` (`bundle`), root `package.json` scripts, lint configs | `release-packaging` | F-51, F-52, F-60 (log setup with S4), F-62, F-63, F-64 |
| **Q · QA + E2E** | S9 | `e2e/**`, `playwright.config.ts`, `src-tauri/crates/infrastructure/tests/integration/**` | `svelte5-frontend`, `run` | e2e per feature, visual baselines, the integration-test harness |
| **R · Review** | S10 | nothing (read-only, comments go in its own memory) | `code-review`, plus one layer skill picked per review | the gate for every lane |

### Role card template (paste into the lead's spawn prompt)

```
You are S2, lead of lane B (System packages) in the app-installer swarm.
Load skills: caveman:caveman, rust-clean-architecture, linux-package-backends. No others.
Read: $SWARM/shared/brief.md, contracts.md, decisions.md, ownership.md, board.md (your rows).
Docs: ARCHITECTURE.md §6, §7, §9.2 · platform/fedora-gnome.md "D-Bus services" · ADR-0004, ADR-0007.
Own paths: <from ownership.md>. Don't edit anything else; for changes elsewhere, write ASK O.
Memory: write ONLY $SWARM/agents/S2.md and $SWARM/agents/S2/tickets/. Caveman prose. ≤60 lines.
Wave goal: <from board>. Your features: <IDs> with the acceptance criteria in FEATURES.md.
Do the hard or design parts yourself. Split mechanical work into Haiku tickets (§5.3) and give
them to O to spawn (write "SPAWN: T-B-04, T-B-05" in your memory), at most 8 open tickets at once.
Integrate worker commits into your worktree, then run the lane suite.
When done: STATUS: review, then the final report (§6.3). Only O pushes.
```

## 5. Work breakdown

### 5.1 Waves (map to the roadmap)

| Wave | Goal / demo | Leads active | Peak Haiku | Gate (O verifies) |
|---|---|---|---|---|
| **W0: Scaffold + contracts** | Repo scaffolded per `folder-structure.md`; CI green; prototype UI running on mocks | S1, S4, S5, S6, S8 (+ S10) | 10 | `pnpm check/lint/test` + cargo checks pass; **`contracts.md` frozen v1** |
| **W1: AppImage end-to-end (M1)** | Double-click AppImage, drag, it appears in the app grid | S1, S3, S4, S5, S6, S7, S9, S10 | 30 | F-01–04, F-20–22, F-25, F-29, F-53, F-54 meet their criteria |
| **W2: RPM via PackageKit (M2)** | Install a real RPM through polkit; cancel springs the icon back | S1, S2, S4, S5, S6, S9, S10 | 30 | F-05, F-10–12, F-23–26 (+ F-31 per D-xxx) |
| **W3: Default handler + packaging (M3)** | The built RPM installs and registers as default | S3, S4, S5, S6, S8, S9, S10 | 20 | F-50–52, F-60, F-62 |
| **W4: Library + uninstall (M4)** | Library window, Trash and Remove | S1, S2, S3, S4, S5, S6, S9, S10 | 40 | F-40–43, F-06, F-07 |
| **W5: Edge cases + polish (M5)** | 1.0 candidate | all 10 | 50 | F-13–15, F-27, F-28, F-55, a11y, visual baselines, F-63, F-64 |

Later work (Ubuntu, i18n, rpm icons) gets its own waves after 1.0, using the same process.

**Critical path inside every wave.** Contracts come first (O + S1 + S4, about 10 % of the wave). Then all lanes run in parallel against the frozen contracts, with mocks unblocking the frontend. Then integration (S4 + S5), Q e2e, R review, and finally O's gate.

### 5.2 W0 in detail (do this first; everything later depends on it)

1. **O** creates `$SWARM`, writes `brief.md`, `ownership.md` and `board.md`, and spawns S1, S4, S5, S6 and S8.
2. **S4** scaffolds the Tauri 2 + SvelteKit app (`pnpm create tauri-app`, SvelteKit, TS), reshapes it to `folder-structure.md`, and adds plugins single-instance and log.
3. **S1** creates the workspace crates and the domain types from ARCHITECTURE §5, port traits from §6, and `application::testing` fakes. Then it drafts contracts.
4. **S4 + S5** draft the DTOs and gateway interfaces from `ipc-contract.md`.
5. **O** reviews the drafts and copies them into **`shared/contracts.md` v1 (frozen)**. After that, any change needs `ASK O` and a new version.
6. **S6** ports the prototype into `src/lib/ui` primitives and tokens. **S5** builds the mock gateways and the installer state machine.
7. **S8** writes the CI workflow, lint configs and the `lint:copy` script.
8. Haiku tickets in W0: crate boilerplate, `Cargo.toml` dependencies, i18n key extraction from COPY.md, per-component skeletons, fixture generator script, vitest setup, prettier/eslint config.

### 5.3 Haiku tickets: rules and format

A ticket goes to Haiku only if **all** of these are true:
- It touches 1–3 files, all named in the ticket.
- Signatures are given exactly (or the ticket points to `contracts.md`).
- One test command proves it.
- It needs no design choice.

Otherwise the lead keeps the work. Good Haiku work: pure mapping functions, enum tables, `From` impls, unit-test tables, fixtures, i18n keys, a single UI primitive from a spec, mock gateway fixtures, lint fixes, doc table updates.

Ticket file `$SWARM/agents/<lead>/tickets/T-<lane>-<nn>.md`:

```
T-B-07 · lane B · lead S2 · haiku
SKILL: linux-package-backends            # exactly one (+ caveman)
GOAL: map PK Status u32 → InstallPhase.
FILES: src-tauri/crates/infrastructure/src/packagekit/mapping.rs (edit only)
READ: skill §"PackageKit adapter" bullet "Progress". contracts.md §InstallPhase.
SIG:  pub fn phase_from_status(status: u32) -> InstallPhase
      pub fn percent_from_pk(p: u32) -> Option<u8>   // 101 → None
TEST: cd src-tauri && cargo test -p installer-infrastructure packagekit::mapping
DONE: test pass. clippy clean on file. no other file touched. commit "feat(infra): F-25 map PK status".
```

Worker spawn prompt (keep it this short):

```
You are H-B-07, a Haiku worker. Load caveman:caveman + the ONE skill named in the ticket.
Do ONLY ticket $SWARM/agents/S2/tickets/T-B-07.md. Touch only the files it lists.
Write ONLY $SWARM/agents/H-B-07.md: STATUS line, SHA, ≤5 lines of notes. Caveman prose.
2 failed tries → STATUS: blocked + why, then stop. Final report ≤60 words.
```

### 5.4 Example ticket pools per wave (leads refine these)

| Lane | W1 Haiku tickets (examples) |
|---|---|
| A | `PackageFormat::sniff` table tests · `AppId::from_name` + proptest · `button_layout::parse` + tests · `Compatibility::evaluate` test table |
| C | ELF shoff offset (32/64-bit) fn + tests · fixture AppImage generator script · desktop-entry escaping fn · icon size selection fn |
| D | `From<domain>` impls per DTO · `CommandError` code mapping table · capability JSON entries |
| E | i18n keys for dmg/phase/done · `copy.ts` exhaustive maps · `format.ts` ETA strings + tests · mock fixtures for 3 formats |
| F | `Button`, `ProgressBar`, `Spinner`, `Toast`, `AppIcon` from DESIGN-SYSTEM §2–3 (one ticket each) |
| G | accent → `--on-accent` contrast fn + tests · `data-*` attribute setter |
| Q | one Playwright spec per acceptance criterion |

## 6. Rules for every agent

### 6.1 Git

- Each agent works in its own worktree branch: `swarm/<agent-id>/<ticket-or-topic>`.
- Workers commit on their branch. The lead merges worker branches into `swarm/lane-<X>`, using `git merge` (no rebasing other agents' branches).
- O merges lanes into `ccr-f9e12bc2-ooggwd` after the gate. **Only O pushes.**
- Commit messages are normal English Conventional Commits with feature IDs (not caveman).

### 6.2 Quality bar (Definition of Done for a feature)

1. Every acceptance criterion in FEATURES.md is met, and Q has an e2e test or a recorded manual check for it.
2. Tests at the lowest layer possible. The lane suite is green. Clippy and lint are clean.
3. CLAUDE.md non-negotiables hold (dependency rule, no executing packages, no password UI, no shelling out, plain language, mocks exist).
4. IPC changes: the DTO, `bindings.ts` (by S4), both gateways and `ipc-contract.md` were updated together.
5. S10 review shows no red findings.
6. O ticks the feature on `board.md`.

### 6.3 Final report template (caveman, ≤120 words; workers ≤60)

```
ID: S2 · T-B-03
DID: PK transaction progress → observer. phases mapped.
FILES: packagekit/transaction.rs, mapping.rs
SHA: 9f3e2a1 (branch swarm/S2/progress)
TEST: cargo test -p installer-infrastructure packagekit → 14 pass
OPEN: WAITING_FOR_AUTH value unverified (ASK O)
```

### 6.4 Things agents cannot verify (mark them HUMAN)

The cloud container has no GNOME session, no polkit dialog, and possibly no PackageKit or podman. Any check that needs them is written as `HUMAN: <check>` in the agent's memory, and O collects these into one list for the user:
- the M0 spikes (window corners/shadow, PackageKit present on fresh Fedora 45, polkit dialog appears)
- the visual feel on GNOME 51 (light/dark, accent, button layout)
- a real install of an RPM and an AppImage, appearing in the app grid, launching
- installing the built RPM on a clean VM

Code must still ship with fakes and tests so that these human checks are only confirmation.

## 7. Orchestrator loop

```
for each wave:
  1. update board.md (lanes, features, leads) → chmod 444
  2. spawn leads (≤10 sonnet), each with its role card
  3. loop until all lanes are STATUS: review:
       a. poll: grep '^STATUS\|^ASK\|^SPAWN\|^HUMAN' $SWARM/agents/*.md
       b. answer ASKs → decisions.md (D-0xx) / contracts.md (vN+1), notify affected agents
       c. spawn requested tickets (keep ≤50 haiku running; queue the rest)
       d. agent blocked twice on the same thing → reassign to its lead or take the decision
       e. otherwise wait for notifications (no sleep-polling)
  4. S10 reviews each lane → leads fix red findings
  5. gate: merge lanes, run full checks, run Q e2e, tick board
  6. commit + push, list HUMAN checks for the user, start the next wave
```

**Scheduling tips:**
- Start a wave with only the leads. Workers come once tickets exist.
- If 5 or more workers are blocked on one contract, pause that lane and fix the contract first.
- Retire leads whose lane is done for the wave, which frees Sonnet slots for S9/S10.
- Never run two agents on the same owned path at the same time. `ownership.md` decides.

## 8. Context packs (what each role reads, and nothing more)

| Role | Read |
|---|---|
| Everyone | `$SWARM/shared/brief.md`, `contracts.md`, `decisions.md`, `CLAUDE.md` |
| S1 | ARCHITECTURE §4–6, state-machines (domain-relevant), ADR-0003 |
| S2 | ARCHITECTURE §7, §9.2, §9.4 · fedora-gnome "D-Bus services" · ADR-0004, ADR-0007 |
| S3 | ARCHITECTURE §7, §9.3 · fedora-gnome "AppImage specifics", "Freedesktop files" · ADR-0005 |
| S4 | ipc-contract.md · ARCHITECTURE §8, §12 (security) · ADR-0009 |
| S5 | ARCHITECTURE §10 · state-machines.md · ipc-contract.md · COPY.md |
| S6 | DESIGN-SYSTEM.md · `design/prototype/index.html` (CSS + markup only) · COPY.md (keys) |
| S7 | DESIGN-SYSTEM §5 · fedora-gnome "GSettings we read" · ADR-0006 |
| S8 | ARCHITECTURE §14 · DEVELOPMENT.md |
| S9 | FEATURES.md (acceptance criteria for the wave) · state-machines.md |
| S10 | the diff + CLAUDE.md non-negotiables + the one layer skill |
| Haiku | its ticket only (plus whatever the ticket's READ line names) |

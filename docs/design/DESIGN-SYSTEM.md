# Design System: "Mac-like, at home on GNOME"

The goal is an installer that feels like a macOS disk-image window, an AppKit utility, while respecting the GNOME desktop it runs on (accent colour, dark style, control placement, reduced motion). The visual reference is [`design/prototype/index.html`](../../design/prototype/index.html). Its tokens are the starting values below.

> **Fonts:** Apple's SF Pro licence only allows use on Apple platforms, so we can't bundle it. Use **Inter** (bundled `.woff2`, OFL), which the prototype already uses, with a fallback stack of `Inter, "Adwaita Sans", Cantarell, system-ui, sans-serif`. Never load fonts from a CDN; the CSP forbids it.

## 1. Principles (taken from Apple's HIG and adapted)

1. **Clarity.** One obvious action per screen (drag). Large icon, short labels, no jargon.
2. **Deference.** Chrome is quiet: neutral greys, hairline separators. The app's own icon is the most colourful thing in the window.
3. **Depth through materials and motion, not decoration.** Sheets slide from the title bar, alerts scale in, icons spring.
4. **Direct manipulation.** The icon follows the pointer one-to-one, and the drop target responds before release.
5. **Consistency with the host.** Accent, scheme and control placement come from GNOME. Spacing, type and motion come from macOS.

## 2. AppKit → our component mapping

| AppKit / macOS concept | Our component | Behaviour notes |
|---|---|---|
| `NSWindow` (titled, non-resizable utility) | `WindowChrome.svelte` | 40 px title bar, centred bold 13 px title, hairline bottom border; drag region on the whole bar except the controls |
| Traffic lights (`standardWindowButton`) | `WindowControls.svelte` | 12 px circles, 8 px gap; glyphs (× − +) appear on hover of the group; inactive window → all grey `#D1D1D6`/`#4A4A4E` (dark). Placement and set follow GNOME `button-layout` |
| Sheet (`beginSheet`) | `Sheet.svelte` | Attached to the bottom of the title bar, 340 px wide, slides down 260 ms, bottom corners 14 px, dims content to 55 % |
| `NSAlert` | `Alert.svelte` | Centred, 300 px, app icon 64 px with an optional badge, bold 13 px message, 12 px informative text, buttons stacked full width (macOS 11+ style) |
| `NSButton` push (default / cancel) | `Button.svelte` | 36 px min height (44 px touch target via padding), radius 8. Primary = accent fill. Secondary = `#E8E8ED`. Destructive = red text on secondary. `Enter` triggers default, `Esc` triggers cancel |
| `NSProgressIndicator` (bar / spinning) | `ProgressBar.svelte`, `Spinner.svelte` | 6 px track, radius 3, accent fill, 140 ms linear width transitions; indeterminate = a sliding 30 % segment |
| `NSUserNotification` banner | `Toast.svelte` | Top-right in the window, 300 px, translucent material, slides in from the right, auto-dismisses after 4 s |
| Finder icon view / Launchpad | `AppGrid.svelte` / `AppTile.svelte` | 96 px icons, 2-line names, selection = rounded tint behind the icon plus an accent pill behind the name (like the prototype's `.file.selected`) |
| Disclosure triangle | `Disclosure.svelte` | "Details" for technical info, collapsed by default |
| DMG background with arrow | `DmgView.svelte` | App icon (112 px) → animated dashed arrow → Applications folder tile (112 px, radius 22) |

## 3. Tokens (`src/lib/ui/tokens.css`)

All colours are tokens on `:root`. Dark values apply under `[data-scheme="dark"]`, which `AppearanceStore` sets from the portal; we don't rely on `prefers-color-scheme` alone.

| Token | Light | Dark | Use |
|---|---|---|---|
| `--accent` | portal accent or `#0B6BD6` | portal accent or `#0A84FF` | Primary buttons, progress, focus, selection |
| `--on-accent` | `#FFFFFF` | `#FFFFFF` | Text on the accent (compute contrast; switch to `#1D1D1F` if the accent is light, e.g. GNOME yellow) |
| `--window-bg` | `#F5F5F7` | `#1E1E20` | Window content |
| `--titlebar-bg` | `#ECECEE` | `#2A2A2D` | Title bar |
| `--separator` | `#D8D8DC` | `#3A3A3D` | Hairlines (0.5 px on HiDPI, 1 px otherwise) |
| `--label` | `#1D1D1F` | `#F5F5F7` | Primary text |
| `--label-secondary` | `#6E6E73` | `#A1A1A6` | Secondary text |
| `--label-tertiary` | `#3A3A3C` | `#D1D1D6` | Headers inside window |
| `--fill-secondary` | `#E8E8ED` | `#3A3A3D` | Secondary buttons |
| `--track` | `#DCDCE1` | `#3A3A3D` | Progress track |
| `--success` | `#2E9E4F` | `#32D74B` | Done check |
| `--danger` | `#B42318` | `#FF453A` | Destructive text, field errors |
| `--light-close` / `--light-min` / `--light-zoom` | `#FF5F57` / `#FEBC2E` / `#28C840` | same | Traffic lights |
| `--material-thin` | `rgba(246,246,248,.78)` + `backdrop-filter: blur(20px) saturate(180%)` | `rgba(40,40,44,.72)` | Toasts, sheets over content |
| `--dim-sheet` | `rgba(245,245,247,.55)` | `rgba(30,30,32,.55)` | Behind sheets |
| `--dim-modal` | `rgba(0,0,0,.28)` | `rgba(0,0,0,.45)` | Behind alerts and the permission shade |

High contrast (`[data-contrast="more"]`): separators go to `--label-secondary`, materials become opaque, and focus rings are 3 px.

### Typography (Inter, macOS text styles)

| Style | Size / line | Weight | Use |
|---|---|---|---|
| `title-2` | 19 / 24 | 700 | "Lumen Notes is installed" |
| `title-3` | 16 / 21 | 700 | "Installing Lumen Notes" |
| `headline` | 13 / 17 | 700 | Window title, alert message, sheet title |
| `body` | 13 / 18 | 400 | Default |
| `callout` | 14 / 20 | 400 | DMG hint ("Drag Lumen Notes to Applications") |
| `subhead` | 12.5 / 18 | 400 | Sheet body |
| `footnote` | 12 / 16 | 400–600 | Progress meta, lock note |
| `caption` | 11 / 14 | 400 | File sub-labels |

Use `font-variant-numeric: tabular-nums` for percentages and times.

### Spacing, radii, elevation

- 4 px grid: 4, 8, 12, 16, 20, 24, 32, 56 (gap between icon and folder).
- Radii: window 12, sheet bottom corners 14, alert 14, button 8, field 7, app-icon squircle 22.5 % of size, folder tile 22.
- Shadows: window `0 30px 70px rgba(0,0,0,.4), 0 0 0 .5px rgba(0,0,0,.3)` (only if the M0 spike allows a transparent margin); sheet `0 16px 36px rgba(0,0,0,.2)`; alert `0 24px 60px rgba(0,0,0,.35)`; dragged icon `drop-shadow(0 14px 18px rgba(0,0,0,.28))`.

## 4. Motion (`src/lib/ui/motion.ts`)

| Name | Spec | Where |
|---|---|---|
| `spring.drag` | `Spring` stiffness 0.2, damping 0.65 | Icon follows the pointer; spring back on cancel |
| `spring.pop` | `cubic-bezier(.2,.9,.3,1.2)` 560 ms | Success check |
| `ease.sheet` | `cubic-bezier(.2,.9,.3,1)` 260 ms | Sheet down/up |
| `ease.alert` | `cubic-bezier(.2,.9,.3,1.1)` 260 ms, scale .92 → 1 | Alert, permission shade |
| `ease.fly` | `cubic-bezier(.4,0,.2,1)` 500 ms, scale → .25, fade | Icon into folder |
| `gulp` | 420 ms scale 1 → 1.14 → 1, delayed 320 ms | Folder after drop |
| `rise` | 320–420 ms translateY 8 → 0, fade | Progress and done views |
| `shake` | 380 ms | Reserved; we don't collect passwords, so it is never used for auth |

**Reduced motion** (`[data-motion="reduce"]`, set from GNOME's `enable-animations=false`, the GNOME 51 Reduced Motion option, or `prefers-reduced-motion`): replace springs with 120 ms cross-fades. The drag still follows the pointer, but there is no fly-in, gulp or pop.

## 5. Window controls and GNOME `button-layout`

`org.gnome.desktop.wm.preferences button-layout` looks like `"appmenu:close"` (GNOME default) or `"close,minimize,maximize:"` (macOS-like). Parse it like this:

1. Split on `:` into left and right. Keep only `close`, `minimize` and `maximize`; ignore `appmenu`, `icon`, `menu` and `spacer`.
2. Render on whichever side holds `close`. Order follows the setting's order, but the visual style is always macOS circles.
3. `maximize` renders as a disabled green circle (our window is not resizable), mirroring a macOS window where zoom is unavailable. Hide it if the setting omits it.
4. The title stays centred in the bar regardless of side.
5. If the setting is unreadable, use `right: [close]` (the GNOME default) so we look native on stock Fedora.

## 6. Layout specs

### Installer (600 × 400)
- Title bar 40 px.
- DMG view: content centred; row gap 56 px; icon and folder columns 128 px wide, 112 px artwork; labels 13/600 underneath; hint text 14 px secondary 34 px below the row; the lock note ("Needs your password") shows for system scope only, 12 px with a lock glyph.
- Progress view: title-3, a 360 px bar, and a meta row (phrase left, time right).
- Done view: 72 px green check, title-2, body secondary (max 380 px), buttons **Open** (primary) and **Done**.

### Library (min 640 × 420, resizable)
- Toolbar inside the title bar: title "Applications", a search field (v1.x) and an **Open a Package…** button.
- Grid with 24 px padding, 112 px cells, 16 px gaps. Right-click or `Delete` → TrashConfirm alert.

## 7. Iconography

- The app icon is a rounded square ("squircle"), as on macOS Big Sur and later. Mask the extracted icon only if it is square and has no transparency; otherwise show it as is (Linux icons often have their own shape).
- The generic package icon is the prototype's `#i-app` symbol, tinted with `--accent`.
- The Applications folder is a custom SVG folder in the accent tint, with an "A" glyph mark.
- Use a lock glyph next to text, never on its own.

## 8. Do / Don't

- ✅ Use sheets for decisions about *this* window (default handler). Use alerts for destructive confirmation.
- ✅ Keep one primary button per view.
- ❌ No custom password field. polkit owns authentication.
- ❌ No file paths, command names, "dnf", "PackageKit", "RPM" or "dependencies" in the UI. See [COPY.md](COPY.md).
- ❌ No Liquid Glass imitation over the whole window. WebKitGTK `backdrop-filter` is costly, so use materials only for small overlays (toast, sheet).

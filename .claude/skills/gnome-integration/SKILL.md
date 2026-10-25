---
name: gnome-integration
description: Make the app follow GNOME 50/51 on Wayland — accent colour, light/dark, high contrast, reduced motion, text scaling, and window-control placement from button-layout — and handle undecorated-window behaviour. Use for appearance code (Rust appearance adapter, AppearanceStore, WindowChrome/WindowControls) and window config.
---

# GNOME integration

References: `docs/platform/fedora-gnome.md`, ADR-0006, DESIGN-SYSTEM §5.

## Appearance source (Rust, `infrastructure/src/appearance.rs`)
| Field | Source | Notes |
|---|---|---|
| `colorScheme` | `ashpd::desktop::settings::Settings::color_scheme()` + `receive_color_scheme_changed()` | `PreferDark` → dark; `NoPreference`/`PreferLight` → light |
| `accent` | `Settings::accent_color()` + change stream | Returns sRGB floats → `#RRGGBB`. Missing on older portals → `None` |
| `highContrast` | `Settings::contrast()` | `High` → true |
| `reducedMotion` | `org.gnome.desktop.interface enable-animations` (via `gio::Settings` or portal `read`) | GNOME 51's Reduced Motion setting ⚠ check which key it writes; also honour CSS `prefers-reduced-motion` |
| `controls` | `gtk::Settings::default().gtk_decoration_layout()` + `connect_gtk_decoration_layout_notify` | GTK3 (Tauri's toolkit) maps GNOME `button-layout`. **Not** in the portal. Must run on the GTK main thread (`app.run_on_main_thread`) |

Merge these into one `Appearance` value. Emit `appearance://changed` on any change, debounced by 50 ms.

## Parsing button-layout
`"appmenu:close"` → `{side:'right', buttons:['close']}`; `"close,minimize,maximize:"` → `{side:'left', buttons:['close','minimize','maximize']}`. Keep only close, minimize and maximize. The side is whichever half contains `close`. Unparseable input → `{right, [close]}`. Unit-test this in domain (it is pure string logic, so it lives in `installer-domain::appearance`).

## Frontend (`features/appearance/AppearanceStore.svelte.ts`)
- Set `document.documentElement.dataset.scheme|contrast|motion|controls`, and `style.setProperty('--accent', …)`. Compute `--on-accent` from luminance (WCAG contrast of at least 4.5 against white, else dark text).
- `WindowControls.svelte` reads `controls` and renders the circles in order. Zoom is disabled. Unfocused windows (`getCurrentWindow().onFocusChanged`) show grey circles.

## Window (tauri.conf.json, window `main`)
`decorations:false, transparent:true, resizable:false, maximizable:false, width:600, height:400, center:true`.
- The title bar gets `data-tauri-drag-region`. Exclude the controls (they are buttons, which is fine). As a fallback, call `getCurrentWindow().startDragging()` on `pointerdown` of the empty bar.
- There is no compositor shadow on undecorated GNOME windows. Follow the decision in ADR-0006 (spike 1) for radius and shadow.
- Double-clicking the title bar does nothing (the window isn't resizable), as on a macOS installer.

## Gotchas
- Wayland-only (GNOME 50+): don't try to set window positions; `center` is a hint.
- WebKitGTK may need `WEBKIT_DISABLE_DMABUF_RENDERER=1` on some GPUs. Document it; don't set it by default.
- Test with: `gsettings set org.gnome.desktop.wm.preferences button-layout 'close,minimize,maximize:'`, `gsettings set org.gnome.desktop.interface color-scheme prefer-dark`, `gsettings set org.gnome.desktop.interface accent-color 'teal'`.

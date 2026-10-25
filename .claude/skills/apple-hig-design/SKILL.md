---
name: apple-hig-design
description: Build or change any visual part of the installer (components, layout, colour, typography, motion) in the macOS/AppKit-inspired style while respecting GNOME appearance settings. Use for anything in src/lib/ui or any .svelte markup and CSS.
---

# Apple HIG / AppKit-style UI for the installer

Source of truth: `docs/design/DESIGN-SYSTEM.md`. Visual reference: `design/prototype/index.html`. Read both before making large changes.

## Workflow
1. Find the AppKit counterpart of what you're building (NSWindow sheet, NSAlert, push button, progress indicator, banner). Use the mapping table in DESIGN-SYSTEM §2. If a primitive already exists in `src/lib/ui`, extend it rather than creating a new one.
2. Use **tokens only** (`var(--accent)`, `var(--label-secondary)`, …). No hex literals in components. If you need a new token, add it to `tokens.css` with both light and dark values (and a high-contrast override if relevant).
3. Use the type ramp (13 px body, 13/700 headline, …), with Inter bundled and `tabular-nums` for numbers.
4. Take motion from `src/lib/ui/motion.ts` presets. Every animation must have a `[data-motion="reduce"]` alternative (cross-fade of 120 ms or less).
5. Check in four states: light/dark × controls left/right. Also check high contrast and 1.25× text.

## Rules
- One primary (accent) button per view. `Enter` triggers default and `Esc` triggers cancel.
- Sheets for window-scoped decisions; centred alerts for destructive confirmations.
- Hit targets of 44 px or more (pad small visuals). Visible `:focus-visible` ring in `--accent`, 2 px, offset 2.
- Use `backdrop-filter` materials only on small overlays (toast, sheet). WebKitGTK blur is expensive.
- No SF Pro, SF Symbols or Apple artwork (licence). Draw our own SVG glyphs.
- Never draw a password field. Authentication belongs to polkit (ADR-0007).
- Window controls are macOS circles, but their side and set come from `Appearance.controls`. Don't hard-code the left side.

## Checklist before done
- [ ] No raw colours or pixel font sizes outside `tokens.css`
- [ ] Dark mode and reduced motion verified (Playwright screenshot or manual)
- [ ] Keyboard path works (Tab order, Enter/Esc)
- [ ] `aria-live="polite"` on changing status text; icons have `aria-hidden` or a label

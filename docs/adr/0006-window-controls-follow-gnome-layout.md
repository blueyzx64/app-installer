# ADR-0006: Mac-style window controls placed by GNOME button-layout

- **Status:** Accepted. Corner and shadow treatment pending the M0 spike.
- **Date:** 2026-10-25

## Context
We want the macOS look, but stock GNOME puts a single close button on the right (`button-layout = 'appmenu:close'`). Hard-coded left traffic lights would feel wrong to GNOME users, while GNOME-drawn decorations would break the macOS look.

## Decision
Use `decorations: false` with our own title bar. Draw macOS-style circular controls, but take their **side and set** from `org.gnome.desktop.wm.preferences button-layout`, read in Rust through GTK (`gtk-decoration-layout`) and pushed to the UI live. The zoom button is disabled because the installer window isn't resizable. Default when unknown: `right: [close]`.

## Consequences
- The window looks Mac-like and still behaves as GNOME users expect.
- We must handle unfocused styling and the drag region ourselves.
- **Spike 1** will choose between (a) `transparent: true` with CSS 12 px radius and no outer shadow, and (b) an opaque square window. Record the result here.

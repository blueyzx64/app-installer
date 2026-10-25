//! R8: window must always be closable (parse docs: "so the window can always be closed").
use installer_domain::appearance::{ControlButton, button_layout};

#[test]
fn review_r8_layout_without_close_still_offers_close() {
    // Valid GNOME layouts that carry no close button. The window has no system decorations,
    // so the only way to close it with the mouse is our own circle.
    for layout in [
        "appmenu:minimize,maximize",
        "minimize,maximize:",
        "minimize:maximize",
        ":maximize",
    ] {
        let c = button_layout::parse(layout);
        assert!(
            c.buttons.contains(&ControlButton::Close),
            "{layout:?} -> {:?}: no close button, window cannot be closed from the chrome",
            c.buttons
        );
    }
}

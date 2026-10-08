//! Sela design tokens for operator chrome: colors, type sizes, bar heights,
//! radii and keyboard-order bands. Neutral light chrome; the dark canvas is
//! reserved for slide content. Content/output state must not share selection
//! colors with chrome selection.
//!
//! Colors are `0xRRGGBB` for `gpui::rgb`. Slide fills and rendered text come
//! from the slide data and never from here.
#![allow(dead_code)]

// Surfaces and text.
pub(crate) const CHROME: u32 = 0xf4f4f3;
pub(crate) const SURFACE: u32 = 0xfafaf9;
pub(crate) const TEXT: u32 = 0x292c30;
pub(crate) const MUTED: u32 = 0x646971;
pub(crate) const FAINT: u32 = 0xa5a9af;
pub(crate) const INACTIVE: u32 = 0x92969c;
pub(crate) const DISABLED: u32 = 0xb0b3b8;

// Borders.
pub(crate) const BORDER: u32 = 0xdcdedc;

// Interaction states.
pub(crate) const HOVER: u32 = 0xe8e9e7;
pub(crate) const PRESSED: u32 = 0xdedfdc;
pub(crate) const ACCENT: u32 = 0x536aca;
/// Keyboard focus fill (the focus ring is a fill, not an outline).
pub(crate) const FOCUS_BG: u32 = 0xdce3fa;
pub(crate) const FOCUS_TEXT: u32 = 0x253c91;
pub(crate) const SELECTED_ROW: u32 = 0xe3e7f3;
pub(crate) const SPLITTER_HOVER: u32 = 0xdde2f2;
pub(crate) const DROP_TARGET: u32 = 0xeef1fa;

// Status.
pub(crate) const ON_SCREEN: u32 = 0xd0453b;
pub(crate) const SENDING: u32 = 0xd99a2b;
pub(crate) const ERROR: u32 = 0xb33232;
/// Lit Black/Clear/Logo button.
pub(crate) const MASK_ON: u32 = 0xf6dfdc;

// Dark canvas (Preview and Live panes, slide thumbnails).
pub(crate) const CANVAS: u32 = 0x202226;
pub(crate) const CANVAS_BORDER: u32 = 0x34373d;
pub(crate) const CANVAS_FOCUS: u32 = 0x2c3550;
pub(crate) const CANVAS_TEXT: u32 = 0xb2b6be;
pub(crate) const DARK_TEXT: u32 = 0xf2f2f2;
pub(crate) const DARK_MUTED: u32 = 0x9ea3ab;
pub(crate) const DARK_ERROR: u32 = 0xf08a80;
pub(crate) const BLACK: u32 = 0x000000;

// Type sizes in logical pixels.
pub(crate) const TEXT_BASE: f32 = 13.;
pub(crate) const TEXT_SMALL: f32 = 12.;
pub(crate) const TEXT_CAPTION: f32 = 11.;
pub(crate) const TEXT_TILE: f32 = 9.;

// Bar and row heights.
/// The EW-style menu row above the toolbar (File, Edit, Live, …).
pub(crate) const MENU_BAR_H: f32 = 28.;
pub(crate) const TOP_BAR_H: f32 = 40.;
pub(crate) const PANE_HEADER_H: f32 = 32.;
pub(crate) const TAB_BAR_H: f32 = 32.;
pub(crate) const STATUS_BAR_H: f32 = 30.;
pub(crate) const PANE_FOOTER_H: f32 = 26.;
pub(crate) const CONTROL_H: f32 = 26.;

// Spacing in logical pixels. The scale equals GPUI's 16 px-rem utilities
// (`*_1` = 4, `*_2` = 8, `*_3` = 12).
pub(crate) const SPACE_XS: f32 = 4.;
pub(crate) const SPACE_S: f32 = 8.;
pub(crate) const SPACE_M: f32 = 12.;
/// Horizontal padding of pane headers, footers and message rows.
pub(crate) const PANE_PAD_X: f32 = SPACE_M;
/// Gap between sibling controls and tiles.
pub(crate) const ROW_GAP: f32 = SPACE_S;
pub(crate) const MENU_ITEM_PAD_X: f32 = SPACE_M;
/// Inset of a menu row from the popup edge.
pub(crate) const MENU_ITEM_MARGIN_X: f32 = SPACE_XS;
pub(crate) const DIALOG_PAD: f32 = SPACE_M;
pub(crate) const DIALOG_GAP: f32 = SPACE_S;

// Radii.
pub(crate) const RADIUS: f32 = 4.;
pub(crate) const RADIUS_SMALL: f32 = 3.;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_header_uses_chrome_fill_border_and_height() {
        assert_eq!(PANE_HEADER_H, 32.);
        assert_eq!(CHROME, 0xf4f4f3);
        assert_eq!(BORDER, 0xdcdedc);
    }

    #[test]
    fn spacing_tokens_match_the_rendered_chrome() {
        assert_eq!((SPACE_XS, SPACE_S, SPACE_M), (4., 8., 12.));
        assert_eq!(PANE_PAD_X, 12.);
        assert_eq!(ROW_GAP, 8.);
        assert_eq!((MENU_ITEM_PAD_X, MENU_ITEM_MARGIN_X), (12., 4.));
        assert_eq!((DIALOG_PAD, DIALOG_GAP), (12., 8.));
    }

    #[test]
    fn selected_row_and_focus_ring_tokens() {
        assert_eq!(SELECTED_ROW, 0xe3e7f3);
        assert_eq!(FOCUS_BG, 0xdce3fa);
        assert_ne!(SELECTED_ROW, FOCUS_BG);
        assert_ne!(SELECTED_ROW, SURFACE);
    }

    #[test]
    fn mask_button_lit_state_is_distinct_from_chrome_states() {
        assert_eq!(MASK_ON, 0xf6dfdc);
        for other in [CHROME, SURFACE, HOVER, PRESSED, FOCUS_BG, SELECTED_ROW] {
            assert_ne!(MASK_ON, other);
        }
    }

    #[test]
    fn operator_modules_hold_no_color_literals() {
        fn walk(dir: &std::path::Path, found: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, found);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    for (n, line) in text.lines().enumerate() {
                        if line.contains("rgb(0x") || line.contains("rgba(0x") {
                            found.push(format!("{}:{}", path.display(), n + 1));
                        }
                    }
                }
            }
        }
        let mut found = Vec::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/operator"),
            &mut found,
        );
        assert!(found.is_empty(), "color literals outside theme: {found:?}");
    }
}

//! Operator Black/Clear/Logo state. No GPUI types and no rendering.
//!
//! Transitions follow EasyWorship 8.0.49 observations EW8-OBS-015–017
//! (docs/reference-observations.md): Black and Logo replace each other, Clear
//! stacks with either, a repeated press turns a mask off, and Go Live, slide
//! navigation and output off/on keep the masks. Live double-click clearing
//! every mask in a combined state is provisional (EW8-OBS-020).

/// What the audience shows over the applied slide.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Layer {
    /// The applied slide, text and background.
    #[default]
    None,
    /// The applied slide's background without its text.
    Clear,
    Black,
    /// The prepared logo image instead of the slide.
    Logo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mask {
    Black,
    Clear,
    Logo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Cover {
    Black,
    Logo,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Masks {
    cover: Option<Cover>,
    clear: bool,
}

impl Masks {
    pub fn is_on(&self, mask: Mask) -> bool {
        match mask {
            Mask::Black => self.cover == Some(Cover::Black),
            Mask::Logo => self.cover == Some(Cover::Logo),
            Mask::Clear => self.clear,
        }
    }

    pub fn any(&self) -> bool {
        self.cover.is_some() || self.clear
    }

    pub fn toggle(&mut self, mask: Mask) {
        match mask {
            Mask::Clear => self.clear = !self.clear,
            Mask::Black => self.toggle_cover(Cover::Black),
            Mask::Logo => self.toggle_cover(Cover::Logo),
        }
    }

    fn toggle_cover(&mut self, cover: Cover) {
        self.cover = if self.cover == Some(cover) {
            None
        } else {
            Some(cover)
        };
    }

    /// Double-click on a Live slide turns the masks off and applies that slide.
    pub fn live_double_click(&mut self) {
        *self = Self::default();
    }

    /// A cover hides a stacked Clear; turning the cover off reveals it again.
    pub fn layer(&self) -> Layer {
        match (self.cover, self.clear) {
            (Some(Cover::Black), _) => Layer::Black,
            (Some(Cover::Logo), _) => Layer::Logo,
            (None, true) => Layer::Clear,
            (None, false) => Layer::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn after(presses: &[Mask]) -> Masks {
        let mut masks = Masks::default();
        for mask in presses {
            masks.toggle(*mask);
        }
        masks
    }

    fn lit(masks: Masks) -> Vec<Mask> {
        [Mask::Black, Mask::Clear, Mask::Logo]
            .into_iter()
            .filter(|m| masks.is_on(*m))
            .collect()
    }

    #[test]
    fn starts_unmasked() {
        let masks = Masks::default();
        assert_eq!(masks.layer(), Layer::None);
        assert!(!masks.any());
    }

    #[test]
    fn repeated_press_toggles_each_mask() {
        for (mask, layer) in [
            (Mask::Black, Layer::Black),
            (Mask::Clear, Layer::Clear),
            (Mask::Logo, Layer::Logo),
        ] {
            let mut masks = Masks::default();
            for step in 0..3 {
                masks.toggle(mask);
                let on = step % 2 == 0;
                assert_eq!(masks.is_on(mask), on);
                assert_eq!(masks.layer(), if on { layer } else { Layer::None });
            }
        }
    }

    /// Ordered pairs M, N, N, M from RUN-W03-2026-10-06.
    #[test]
    fn ordered_pairs_match_the_observed_table() {
        use Layer as L;
        use Mask::*;
        let cases = [
            (Black, Clear, [L::Black, L::Black, L::Black, L::None]),
            (Black, Logo, [L::Black, L::Logo, L::None, L::Black]),
            (Clear, Black, [L::Clear, L::Black, L::Clear, L::None]),
            (Clear, Logo, [L::Clear, L::Logo, L::Clear, L::None]),
            (Logo, Black, [L::Logo, L::Black, L::None, L::Logo]),
            (Logo, Clear, [L::Logo, L::Logo, L::Logo, L::None]),
        ];
        for (first, second, layers) in cases {
            let mut masks = Masks::default();
            for (press, expected) in [first, second, second, first].into_iter().zip(layers) {
                masks.toggle(press);
                assert_eq!(masks.layer(), expected, "{first:?} {second:?} {press:?}");
            }
        }
    }

    #[test]
    fn black_and_logo_replace_each_other_and_clear_stacks() {
        assert_eq!(lit(after(&[Mask::Black, Mask::Logo])), [Mask::Logo]);
        assert_eq!(lit(after(&[Mask::Logo, Mask::Black])), [Mask::Black]);
        assert_eq!(
            lit(after(&[Mask::Clear, Mask::Black])),
            [Mask::Black, Mask::Clear]
        );
        assert_eq!(
            lit(after(&[Mask::Logo, Mask::Clear])),
            [Mask::Clear, Mask::Logo]
        );
    }

    /// Every reachable state, from the six ordered triples and their reversal.
    #[test]
    fn black_with_logo_is_unreachable() {
        use Mask::*;
        let orders = [
            [Black, Clear, Logo],
            [Black, Logo, Clear],
            [Clear, Black, Logo],
            [Clear, Logo, Black],
            [Logo, Black, Clear],
            [Logo, Clear, Black],
        ];
        let mut seen = std::collections::HashSet::new();
        for order in orders {
            let mut masks = Masks::default();
            for press in order.iter().chain(order.iter().rev()) {
                masks.toggle(*press);
                assert!(!(masks.is_on(Black) && masks.is_on(Logo)));
                seen.insert(lit(masks));
            }
        }
        assert_eq!(seen.len(), 6, "{seen:?}");
    }

    /// Triple Clear, Black, Logo then reverse, as recorded.
    #[test]
    fn observed_triple_sequence() {
        use Mask::*;
        let mut masks = Masks::default();
        let layers: Vec<_> = [Clear, Black, Logo, Logo, Black, Clear]
            .into_iter()
            .map(|press| {
                masks.toggle(press);
                masks.layer()
            })
            .collect();
        assert_eq!(
            layers,
            [
                Layer::Clear,
                Layer::Black,
                Layer::Logo,
                Layer::Clear,
                Layer::Black,
                Layer::Black
            ]
        );
    }

    #[test]
    fn live_double_click_turns_every_mask_off() {
        let mut masks = after(&[Mask::Clear, Mask::Logo]);
        masks.live_double_click();
        assert_eq!(masks, Masks::default());
        assert_eq!(masks.layer(), Layer::None);
    }
}

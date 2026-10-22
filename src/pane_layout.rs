//! Persisted operator layout: splitter proportions, Resources collapsed state
//! and main window placement. Framework-independent so parsing and the
//! offscreen correction are unit-testable.
//!
//! Stored in `settings.txt` as `pane_ratios=a,b,c`, `resources_collapsed=0|1`
//! and `window=x,y,width,height[,max]`. Any malformed or out-of-range value
//! drops the whole layout back to the default instead of failing the file.

/// Proportions are stored in ten-thousandths so a restored splitter lands
/// well within a pixel of where it was dragged.
pub const RATIO_SCALE: u16 = 10_000;
/// Default Schedule/Preview/Live and Resources proportions.
pub const DEFAULT_RATIOS: [u16; 3] = [2400, 6200, 6200];
pub const MIN_WINDOW: (u32, u32) = (720, 440);
const MAX_EXTENT: u32 = 30_000;
const MAX_ORIGIN: i32 = 100_000;
/// A window needs at least this much of itself on a monitor to count as reachable.
const REACH: (i32, i32) = (200, 100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub ratios: [u16; 3],
    pub collapsed: bool,
    pub window: Option<Placement>,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            ratios: DEFAULT_RATIOS,
            collapsed: false,
            window: None,
        }
    }
}

impl Layout {
    pub fn from_fractions(ratios: [f32; 3]) -> [u16; 3] {
        ratios.map(|r| (r.clamp(0., 1.) * f32::from(RATIO_SCALE)).round() as u16)
    }

    pub fn fractions(&self) -> [f32; 3] {
        self.ratios.map(|r| f32::from(r) / f32::from(RATIO_SCALE))
    }

    pub fn render(&self) -> String {
        let [a, b, c] = self.ratios;
        let mut text = format!(
            "pane_ratios={a},{b},{c}\nresources_collapsed={}\n",
            u8::from(self.collapsed)
        );
        if let Some(w) = self.window {
            text.push_str(&format!(
                "window={},{},{},{}{}\n",
                w.x,
                w.y,
                w.width,
                w.height,
                if w.maximized { ",max" } else { "" }
            ));
        }
        text
    }
}

/// Collects layout keys while the settings file is parsed.
#[derive(Default)]
pub struct Parts {
    ratios: Option<[u16; 3]>,
    collapsed: Option<bool>,
    window: Option<Placement>,
    seen: bool,
    bad: bool,
}

impl Parts {
    /// `true` when `key` belongs to the layout.
    pub fn accept(&mut self, key: &str, value: &str) -> bool {
        match key {
            "pane_ratios" => self.ratios = parse_ratios(value),
            "resources_collapsed" => {
                self.collapsed = match value {
                    "0" => Some(false),
                    "1" => Some(true),
                    _ => None,
                }
            }
            "window" => self.window = parse_window(value),
            _ => return false,
        }
        let ok = match key {
            "pane_ratios" => self.ratios.is_some(),
            "resources_collapsed" => self.collapsed.is_some(),
            _ => self.window.is_some(),
        };
        self.seen = true;
        self.bad |= !ok;
        true
    }

    pub fn finish(self) -> Option<Layout> {
        (self.seen && !self.bad).then(|| Layout {
            ratios: self.ratios.unwrap_or(DEFAULT_RATIOS),
            collapsed: self.collapsed.unwrap_or(false),
            window: self.window,
        })
    }
}

fn parse_ratios(value: &str) -> Option<[u16; 3]> {
    let mut parts = value.split(',').map(|p| p.trim().parse::<u16>().ok());
    let a = parts.next()??;
    let b = parts.next()??;
    let c = parts.next()??;
    if parts.next().is_some() {
        return None;
    }
    // Splitters must keep three non-empty columns and a non-empty top row.
    (a > 0 && a < b && b < RATIO_SCALE && c > 0 && c < RATIO_SCALE).then_some([a, b, c])
}

fn parse_window(value: &str) -> Option<Placement> {
    let mut parts = value.split(',');
    let x: i32 = parts.next()?.trim().parse().ok()?;
    let y: i32 = parts.next()?.trim().parse().ok()?;
    let width: u32 = parts.next()?.trim().parse().ok()?;
    let height: u32 = parts.next()?.trim().parse().ok()?;
    let maximized = match parts.next() {
        None => false,
        Some("max") => true,
        Some(_) => return None,
    };
    if parts.next().is_some()
        || !(-MAX_ORIGIN..=MAX_ORIGIN).contains(&x)
        || !(-MAX_ORIGIN..=MAX_ORIGIN).contains(&y)
        || !(1..=MAX_EXTENT).contains(&width)
        || !(1..=MAX_EXTENT).contains(&height)
    {
        return None;
    }
    Some(Placement {
        x,
        y,
        width,
        height,
        maximized,
    })
}

/// A monitor's usable (work) area in the same logical pixels as `Placement`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    fn overlap(&self, p: &Placement) -> (i32, i32) {
        let w = (self.x + self.width as i32).min(p.x + p.width as i32) - self.x.max(p.x);
        let h = (self.y + self.height as i32).min(p.y + p.height as i32) - self.y.max(p.y);
        (w.max(0), h.max(0))
    }
}

/// Returns a placement that is fully usable: at least the minimum size, no
/// larger than its monitor, and reachable on a connected monitor. A saved
/// rectangle that is missing or offscreen is centred on the primary area.
/// Also returns the index of the area it belongs to, so the window opens on
/// that monitor; `None` only when no monitor is known.
pub fn fit_placement(
    saved: Placement,
    areas: &[Area],
    primary: usize,
) -> (Placement, Option<usize>) {
    if areas.is_empty() {
        return (saved, None);
    }
    let fallback = if primary < areas.len() { primary } else { 0 };
    let reachable = areas
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            let (w, h) = a.overlap(&saved);
            w >= REACH.0.min(saved.width as i32)
                && h >= REACH.1.min(saved.height as i32)
                && saved.y >= a.y
        })
        .max_by_key(|(_, a)| {
            let (w, h) = a.overlap(&saved);
            i64::from(w) * i64::from(h)
        })
        .map(|(index, _)| index);
    let index = reachable.unwrap_or(fallback);
    let home = areas[index];
    let width = saved
        .width
        .max(MIN_WINDOW.0)
        .min(home.width.max(MIN_WINDOW.0));
    let height = saved
        .height
        .max(MIN_WINDOW.1)
        .min(home.height.max(MIN_WINDOW.1));
    let (x, y) = if reachable.is_some() {
        (saved.x, saved.y)
    } else {
        (
            home.x + (home.width as i32 - width as i32) / 2,
            home.y + (home.height as i32 - height as i32) / 2,
        )
    };
    let fitted = Placement {
        x,
        y,
        width,
        height,
        maximized: saved.maximized,
    };
    (fitted, Some(index))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(lines: &[(&str, &str)]) -> Option<Layout> {
        let mut parts = Parts::default();
        for (k, v) in lines {
            assert!(parts.accept(k, v));
        }
        parts.finish()
    }

    const PRIMARY: Area = Area {
        x: 0,
        y: 0,
        width: 2560,
        height: 1400,
    };
    const SPARE: Area = Area {
        x: 2560,
        y: 0,
        width: 1920,
        height: 1040,
    };

    fn at(x: i32, y: i32, width: u32, height: u32) -> Placement {
        Placement {
            x,
            y,
            width,
            height,
            maximized: false,
        }
    }

    #[test]
    fn layout_round_trips_through_text() {
        let layout = Layout {
            ratios: [2100, 5900, 4400],
            collapsed: true,
            window: Some(Placement {
                maximized: true,
                ..at(-20, 30, 1280, 800)
            }),
        };
        let mut parts = Parts::default();
        for line in layout.render().lines() {
            let (k, v) = line.split_once('=').unwrap();
            assert!(parts.accept(k, v));
        }
        assert_eq!(parts.finish(), Some(layout));
    }

    #[test]
    fn absent_layout_is_none_and_unrelated_keys_are_not_taken() {
        assert_eq!(Parts::default().finish(), None);
        assert!(!Parts::default().accept("language", "id"));
    }

    #[test]
    fn malformed_or_out_of_range_values_drop_the_whole_layout() {
        for (key, value) in [
            ("pane_ratios", "{\"left\":0.2}"),
            ("pane_ratios", "0,5000,5000"),
            ("pane_ratios", "6000,5000,5000"),
            ("pane_ratios", "2000,6000,10000"),
            ("pane_ratios", "2000,6000"),
            ("pane_ratios", "2000,6000,5000,1"),
            ("pane_ratios", "-1,6000,5000"),
            ("resources_collapsed", "yes"),
            ("window", "0,0,0,800"),
            ("window", "0,0,1280"),
            ("window", "0,0,1280,800,huge"),
            ("window", "9999999,0,1280,800"),
        ] {
            assert_eq!(parts(&[(key, value)]), None, "{key}={value}");
        }
        assert_eq!(
            parts(&[("resources_collapsed", "1"), ("pane_ratios", "nope")]),
            None,
            "one bad key discards the good ones"
        );
    }

    #[test]
    fn extreme_window_values_drop_the_layout_without_panicking() {
        for value in [
            "-2147483648,0,1280,800",
            "0,-2147483648,1280,800",
            "2147483647,0,1280,800",
            "0,2147483647,1280,800",
            "-100001,0,1280,800",
            "0,0,4294967295,800",
            "0,0,1280,4294967296",
        ] {
            assert_eq!(parts(&[("window", value)]), None, "window={value}");
        }
        let edge = parts(&[("window", "-100000,100000,1280,800")]).unwrap();
        assert_eq!(edge.window, Some(at(-100_000, 100_000, 1280, 800)));
    }

    #[test]
    fn missing_keys_keep_defaults() {
        let layout = parts(&[("resources_collapsed", "1")]).unwrap();
        assert_eq!(layout.ratios, DEFAULT_RATIOS);
        assert!(layout.collapsed);
        assert_eq!(layout.window, None);
    }

    #[test]
    fn fractions_round_trip_within_a_tenth_of_a_pixel() {
        let stored = Layout::from_fractions([0.2437, 0.6, 0.123_45]);
        let layout = Layout {
            ratios: stored,
            ..Layout::default()
        };
        let back = layout.fractions();
        assert!((back[0] - 0.2437).abs() < 1e-4 && (back[2] - 0.1235).abs() < 1e-4);
    }

    #[test]
    fn visible_placement_is_kept() {
        let saved = at(100, 80, 1280, 800);
        assert_eq!(fit_placement(saved, &[PRIMARY, SPARE], 0), (saved, Some(0)));
    }

    #[test]
    fn a_placement_wholly_on_the_spare_monitor_belongs_to_that_monitor() {
        let on_spare = at(2700, 50, 1000, 700);
        assert_eq!(
            fit_placement(on_spare, &[PRIMARY, SPARE], 0),
            (on_spare, Some(1))
        );
        assert_eq!(
            fit_placement(on_spare, &[SPARE, PRIMARY], 1),
            (on_spare, Some(0))
        );
    }

    #[test]
    fn offscreen_or_missing_monitor_placement_moves_to_the_primary_area() {
        for saved in [
            at(9000, 9000, 1280, 800),
            at(-5000, -4000, 1280, 800),
            at(2700, 50, 1000, 700),
        ] {
            let areas = if saved.x == 2700 {
                vec![PRIMARY]
            } else {
                vec![PRIMARY, SPARE]
            };
            let (fitted, area) = fit_placement(saved, &areas, 0);
            assert_eq!(area, Some(0), "{saved:?}");
            assert!(fitted.x >= 0 && fitted.y >= 0, "{fitted:?}");
            assert!(fitted.x + fitted.width as i32 <= 2560, "{fitted:?}");
            assert!(fitted.y + fitted.height as i32 <= 1400, "{fitted:?}");
            assert_eq!((fitted.width, fitted.height), (saved.width, saved.height));
        }
    }

    #[test]
    fn size_is_clamped_to_the_minimum_and_the_monitor() {
        let (tiny, _) = fit_placement(at(100, 100, 50, 40), &[PRIMARY], 0);
        assert_eq!((tiny.width, tiny.height), MIN_WINDOW);
        let (huge, _) = fit_placement(at(100, 100, 9000, 9000), &[PRIMARY], 0);
        assert_eq!((huge.width, huge.height), (2560, 1400));
    }

    #[test]
    fn title_bar_above_every_monitor_is_not_reachable() {
        let (fitted, _) = fit_placement(at(100, -400, 1280, 800), &[PRIMARY], 0);
        assert!(fitted.y >= 0);
    }

    #[test]
    fn no_known_monitors_keeps_the_saved_placement() {
        let saved = at(5, 5, 900, 600);
        assert_eq!(fit_placement(saved, &[], 0), (saved, None));
    }
}

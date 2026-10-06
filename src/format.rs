//! Per-slide text formatting (EW8-OBS-028..030, EW8-OBS-033). No GPUI types.
//!
//! Every field is an override: `None` keeps the default look, so a song
//! without formatting renders exactly as before. Some values are explicit
//! "off" settings (`Outline { enabled: false, .. }`) that stay distinct from
//! `None` so a later theme default cannot turn them back on.

/// Largest family name kept; installed names are far shorter.
pub const MAX_FONT_NAME: usize = 128;
/// Same cap as the audience text preparer.
pub const MAX_FIXED_SIZE: u16 = 288;
pub const MAX_OUTLINE_SIZE: u8 = 50;
pub const MAX_SHADOW_OFFSET: u8 = 100;
pub const MAX_SHADOW_BLUR: u8 = 50;
pub const MAX_OPACITY: u8 = 100;
/// Encoded size bound, checked by storage before decoding.
pub const MAX_ENCODED: usize = 256;

const CODEC: u8 = 1;
const FIELDS: u16 = 10;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Size {
    /// EW "Resize text to fit element".
    Auto,
    /// EW "Do not auto size text" with this point size.
    Fixed(u16),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Middle,
    Bottom,
}

/// EW outline type None/Outer; Center and Inner are not supported yet.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Outline {
    pub enabled: bool,
    pub color: [u8; 3],
    pub size: u8,
    pub opacity: u8,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Shadow {
    pub enabled: bool,
    pub color: [u8; 3],
    /// Degrees, EW dial convention (315 = down-right on the observed theme).
    pub angle: u16,
    pub offset: u8,
    pub blur: u8,
    pub opacity: u8,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
pub struct SlideFormat {
    pub font: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub size: Option<Size>,
    pub color: Option<[u8; 3]>,
    pub align: Option<Align>,
    pub valign: Option<VAlign>,
    pub outline: Option<Outline>,
    pub shadow: Option<Shadow>,
}

impl SlideFormat {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn is_valid(&self) -> bool {
        let font = self.font.as_deref().is_none_or(|name| {
            !name.trim().is_empty()
                && name.len() <= MAX_FONT_NAME
                && !name.chars().any(char::is_control)
        });
        let size = !matches!(self.size, Some(Size::Fixed(s)) if !(1..=MAX_FIXED_SIZE).contains(&s));
        let outline = self
            .outline
            .is_none_or(|o| (1..=MAX_OUTLINE_SIZE).contains(&o.size) && o.opacity <= MAX_OPACITY);
        let shadow = self.shadow.is_none_or(|s| {
            s.angle < 360
                && s.offset <= MAX_SHADOW_OFFSET
                && s.blur <= MAX_SHADOW_BLUR
                && s.opacity <= MAX_OPACITY
        });
        font && size && outline && shadow
    }

    /// Canonical bytes: codec, presence mask, then present fields in order.
    /// Callers store nothing for the default format.
    pub fn encode(&self) -> Vec<u8> {
        let mut mask = 0u16;
        let mut body = Vec::new();
        let mut field = |bit: u16, present: bool| {
            if present {
                mask |= 1 << bit;
            }
            present
        };
        if field(0, self.font.is_some()) {
            let name = self.font.as_deref().unwrap_or_default().as_bytes();
            body.push(name.len() as u8);
            body.extend_from_slice(name);
        }
        for (bit, flag) in [(1, self.bold), (2, self.italic), (3, self.underline)] {
            if field(bit, flag.is_some()) {
                body.push(u8::from(flag == Some(true)));
            }
        }
        if field(4, self.size.is_some()) {
            match self.size {
                Some(Size::Fixed(size)) => {
                    body.push(1);
                    body.extend_from_slice(&size.to_le_bytes());
                }
                _ => body.push(0),
            }
        }
        if let Some(color) = self.color.filter(|_| field(5, true)) {
            body.extend_from_slice(&color);
        }
        if let Some(align) = self.align.filter(|_| field(6, true)) {
            body.push(align as u8);
        }
        if let Some(valign) = self.valign.filter(|_| field(7, true)) {
            body.push(valign as u8);
        }
        if let Some(o) = self.outline.filter(|_| field(8, true)) {
            body.push(u8::from(o.enabled));
            body.extend_from_slice(&o.color);
            body.extend_from_slice(&[o.size, o.opacity]);
        }
        if let Some(s) = self.shadow.filter(|_| field(9, true)) {
            body.push(u8::from(s.enabled));
            body.extend_from_slice(&s.color);
            body.extend_from_slice(&s.angle.to_le_bytes());
            body.extend_from_slice(&[s.offset, s.blur, s.opacity]);
        }
        let mut out = vec![CODEC];
        out.extend_from_slice(&mask.to_le_bytes());
        out.extend_from_slice(&body);
        out
    }

    /// Strict inverse of `encode`: unknown codec or fields, trailing bytes,
    /// out-of-range values and the (never stored) default are all rejected.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_ENCODED {
            return None;
        }
        let mut r = Reader(bytes);
        if r.u8()? != CODEC {
            return None;
        }
        let mask = u16::from_le_bytes(r.take(2)?.try_into().ok()?);
        if mask == 0 || mask >> FIELDS != 0 {
            return None;
        }
        let has = |bit: u16| mask & (1 << bit) != 0;
        let mut format = Self::default();
        if has(0) {
            let len = usize::from(r.u8()?);
            format.font = Some(std::str::from_utf8(r.take(len)?).ok()?.to_owned());
        }
        for bit in 1..=3 {
            if has(bit) {
                let value = Some(r.flag()?);
                match bit {
                    1 => format.bold = value,
                    2 => format.italic = value,
                    _ => format.underline = value,
                }
            }
        }
        if has(4) {
            format.size = Some(match r.u8()? {
                0 => Size::Auto,
                1 => Size::Fixed(u16::from_le_bytes(r.take(2)?.try_into().ok()?)),
                _ => return None,
            });
        }
        if has(5) {
            format.color = Some(r.rgb()?);
        }
        if has(6) {
            format.align = Some(match r.u8()? {
                0 => Align::Left,
                1 => Align::Center,
                2 => Align::Right,
                _ => return None,
            });
        }
        if has(7) {
            format.valign = Some(match r.u8()? {
                0 => VAlign::Top,
                1 => VAlign::Middle,
                2 => VAlign::Bottom,
                _ => return None,
            });
        }
        if has(8) {
            format.outline = Some(Outline {
                enabled: r.flag()?,
                color: r.rgb()?,
                size: r.u8()?,
                opacity: r.u8()?,
            });
        }
        if has(9) {
            format.shadow = Some(Shadow {
                enabled: r.flag()?,
                color: r.rgb()?,
                angle: u16::from_le_bytes(r.take(2)?.try_into().ok()?),
                offset: r.u8()?,
                blur: r.u8()?,
                opacity: r.u8()?,
            });
        }
        (r.0.is_empty() && format.is_valid()).then_some(format)
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, len: usize) -> Option<&[u8]> {
        let (head, rest) = self.0.split_at_checked(len)?;
        self.0 = rest;
        Some(head)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    fn flag(&mut self) -> Option<bool> {
        match self.u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    fn rgb(&mut self) -> Option<[u8; 3]> {
        self.take(3)?.try_into().ok()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn full() -> SlideFormat {
        SlideFormat {
            font: Some("Tahoma".into()),
            bold: Some(true),
            italic: Some(false),
            underline: Some(true),
            size: Some(Size::Fixed(78)),
            color: Some([255, 128, 0]),
            align: Some(Align::Right),
            valign: Some(VAlign::Bottom),
            outline: Some(Outline {
                enabled: true,
                color: [0, 0, 0],
                size: 7,
                opacity: 100,
            }),
            shadow: Some(Shadow {
                enabled: true,
                color: [0, 0, 0],
                angle: 315,
                offset: 18,
                blur: 9,
                opacity: 90,
            }),
        }
    }

    #[test]
    fn every_field_round_trips_alone_and_together() {
        let all = full();
        assert_eq!(SlideFormat::decode(&all.encode()), Some(all.clone()));
        let singles = [
            SlideFormat {
                font: all.font.clone(),
                ..Default::default()
            },
            SlideFormat {
                bold: Some(false),
                ..Default::default()
            },
            SlideFormat {
                italic: Some(true),
                ..Default::default()
            },
            SlideFormat {
                underline: Some(false),
                ..Default::default()
            },
            SlideFormat {
                size: Some(Size::Auto),
                ..Default::default()
            },
            SlideFormat {
                color: all.color,
                ..Default::default()
            },
            SlideFormat {
                align: Some(Align::Left),
                ..Default::default()
            },
            SlideFormat {
                valign: Some(VAlign::Top),
                ..Default::default()
            },
            SlideFormat {
                outline: Some(Outline {
                    enabled: false,
                    ..all.outline.unwrap()
                }),
                ..Default::default()
            },
            SlideFormat {
                shadow: all.shadow,
                ..Default::default()
            },
        ];
        for (bit, format) in singles.iter().enumerate() {
            let bytes = format.encode();
            assert_eq!(u16::from_le_bytes([bytes[1], bytes[2]]), 1 << bit);
            assert_eq!(SlideFormat::decode(&bytes).as_ref(), Some(format));
        }
        let unicode = SlideFormat {
            font: Some("思源黑体 Ünïcode".into()),
            ..Default::default()
        };
        assert_eq!(SlideFormat::decode(&unicode.encode()), Some(unicode));
    }

    #[test]
    fn decode_rejects_default_unknown_truncated_trailing_and_out_of_range() {
        let bytes = full().encode();
        assert_eq!(SlideFormat::decode(&SlideFormat::default().encode()), None);
        assert_eq!(SlideFormat::decode(&[]), None);
        let mut codec = bytes.clone();
        codec[0] = 2;
        assert_eq!(SlideFormat::decode(&codec), None);
        let mut unknown = bytes.clone();
        unknown[2] |= 0x04;
        assert_eq!(SlideFormat::decode(&unknown), None);
        for len in 0..bytes.len() {
            assert_eq!(SlideFormat::decode(&bytes[..len]), None, "{len}");
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert_eq!(SlideFormat::decode(&trailing), None);
        assert_eq!(SlideFormat::decode(&vec![1; MAX_ENCODED + 1]), None);
        // flag 2, alignment 3, size tag 2, fixed size 0 and 289, bad UTF-8.
        for bytes in [
            vec![1, 0b10, 0, 2],
            vec![1, 0b100_0000, 0, 3],
            vec![1, 0b1_0000, 0, 2],
            vec![1, 0b1_0000, 0, 1, 0, 0],
            vec![1, 0b1_0000, 0, 1, 33, 1],
            vec![1, 1, 0, 2, 0xff, 0xfe],
        ] {
            assert_eq!(SlideFormat::decode(&bytes), None, "{bytes:?}");
        }
    }

    #[test]
    fn validation_bounds() {
        let base = full();
        assert!(base.is_valid());
        let invalid = [
            SlideFormat {
                font: Some(" ".into()),
                ..base.clone()
            },
            SlideFormat {
                font: Some("a".repeat(MAX_FONT_NAME + 1)),
                ..base.clone()
            },
            SlideFormat {
                font: Some("Tab\tName".into()),
                ..base.clone()
            },
            SlideFormat {
                size: Some(Size::Fixed(0)),
                ..base.clone()
            },
            SlideFormat {
                size: Some(Size::Fixed(MAX_FIXED_SIZE + 1)),
                ..base.clone()
            },
            SlideFormat {
                outline: Some(Outline {
                    size: 0,
                    ..base.outline.unwrap()
                }),
                ..base.clone()
            },
            SlideFormat {
                outline: Some(Outline {
                    opacity: MAX_OPACITY + 1,
                    ..base.outline.unwrap()
                }),
                ..base.clone()
            },
            SlideFormat {
                shadow: Some(Shadow {
                    angle: 360,
                    ..base.shadow.unwrap()
                }),
                ..base.clone()
            },
            SlideFormat {
                shadow: Some(Shadow {
                    offset: MAX_SHADOW_OFFSET + 1,
                    ..base.shadow.unwrap()
                }),
                ..base.clone()
            },
            SlideFormat {
                shadow: Some(Shadow {
                    blur: MAX_SHADOW_BLUR + 1,
                    ..base.shadow.unwrap()
                }),
                ..base.clone()
            },
        ];
        for format in invalid {
            assert!(!format.is_valid(), "{format:?}");
            assert_eq!(SlideFormat::decode(&format.encode()), None, "{format:?}");
        }
        assert!(full().encode().len() <= MAX_ENCODED);
        let longest = SlideFormat {
            font: Some("a".repeat(MAX_FONT_NAME)),
            ..full()
        };
        assert!(longest.encode().len() <= MAX_ENCODED);
    }
}

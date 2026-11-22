use regex::Regex;
use std::{ops::Range, sync::LazyLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rgba(pub [u8; 4]);

impl Rgba {
    pub(crate) fn hex(source: &str) -> Option<Self> {
        let digits = source.strip_prefix('#').unwrap_or(source);
        if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let mut channels = [255; 4];
        match digits.len() {
            3 | 4 => {
                for (channel, digit) in channels.iter_mut().zip(digits.chars()) {
                    *channel = digit.to_digit(16)? as u8 * 17;
                }
            }
            6 | 8 => {
                for (index, channel) in channels.iter_mut().take(digits.len() / 2).enumerate() {
                    *channel =
                        u8::from_str_radix(digits.get(index * 2..index * 2 + 2)?, 16).ok()?;
                }
            }
            _ => return None,
        }
        Some(Self(channels))
    }

    pub(crate) fn css(self, original: &str) -> String {
        let [r, g, b, a] = self.0;
        let alpha = a != 255 || original.starts_with('#') && matches!(original.len(), 5 | 9);
        let count = if alpha { 4 } else { 3 };
        let mut text = if matches!(original.len(), 4 | 5)
            && original.starts_with('#')
            && self.0[..count].iter().all(|byte| byte % 17 == 0)
        {
            if alpha {
                format!("#{:x}{:x}{:x}{:x}", r / 17, g / 17, b / 17, a / 17)
            } else {
                format!("#{:x}{:x}{:x}", r / 17, g / 17, b / 17)
            }
        } else if alpha {
            format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
        } else {
            format!("#{r:02x}{g:02x}{b:02x}")
        };
        if original.starts_with('#')
            && original.bytes().any(|byte| byte.is_ascii_uppercase())
            && !original.bytes().any(|byte| byte.is_ascii_lowercase())
        {
            text.make_ascii_uppercase();
        }
        text
    }

    pub(crate) fn hsv(self) -> [f32; 3] {
        let [red, green, blue, _] = self.0.map(|byte| f32::from(byte) / 255.);
        let value = red.max(green).max(blue);
        let delta = value - red.min(green).min(blue);
        if delta == 0. {
            return [0., 0., value];
        }
        let sector = if value == red {
            (green - blue) / delta
        } else if value == green {
            (blue - red) / delta + 2.
        } else {
            (red - green) / delta + 4.
        };
        [sector.rem_euclid(6.) / 6., delta / value, value]
    }

    pub(crate) fn from_hsv([hue, saturation, value]: [f32; 3], alpha: u8) -> Self {
        let chroma = value * saturation;
        Self::from_chroma(hue, chroma, value - chroma, alpha)
    }

    fn from_chroma(hue: f32, chroma: f32, offset: f32, alpha: u8) -> Self {
        let hue = hue.rem_euclid(1.) * 6.;
        let second = chroma * (1. - (hue % 2. - 1.).abs());
        let [red, green, blue] = match hue as u8 {
            0 => [chroma, second, 0.],
            1 => [second, chroma, 0.],
            2 => [0., chroma, second],
            3 => [0., second, chroma],
            4 => [second, 0., chroma],
            _ => [chroma, 0., second],
        };
        Self([
            byte(red + offset),
            byte(green + offset),
            byte(blue + offset),
            alpha,
        ])
    }
}

fn byte(unit: f32) -> u8 {
    (unit.clamp(0., 1.) * 255.).round() as u8
}

pub(crate) fn number(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

fn percentage(value: &str) -> Option<f32> {
    Some(number(value.strip_suffix('%')?)? / 100.)
}

fn functional(source: &str) -> Option<Rgba> {
    let (name, args) = source.strip_suffix(')')?.split_once('(')?;
    let (channels, alpha) = if args.contains(',') {
        let mut parts = args.split(',').map(str::trim);
        let channels = [parts.next()?, parts.next()?, parts.next()?];
        let alpha = parts.next();
        if parts.next().is_some() {
            return None;
        }
        (channels, alpha)
    } else {
        let (channels, alpha) = args
            .split_once('/')
            .map_or((args, None), |(channels, alpha)| {
                (channels, Some(alpha.trim()))
            });
        let mut parts = channels.split_ascii_whitespace();
        let channels = [parts.next()?, parts.next()?, parts.next()?];
        if parts.next().is_some() {
            return None;
        }
        (channels, alpha)
    };
    let alpha = if let Some(alpha) = alpha {
        byte(percentage(alpha).or_else(|| number(alpha))?)
    } else {
        255
    };
    if name.eq_ignore_ascii_case("rgb") || name.eq_ignore_ascii_case("rgba") {
        let channel = |value| {
            percentage(value)
                .or_else(|| Some(number(value)? / 255.))
                .map(byte)
        };
        return Some(Rgba([
            channel(channels[0])?,
            channel(channels[1])?,
            channel(channels[2])?,
            alpha,
        ]));
    }
    let angle = channels[0].to_ascii_lowercase();
    let hue = if let Some(turns) = angle.strip_suffix("turn") {
        number(turns)?
    } else if let Some(radians) = angle.strip_suffix("rad") {
        number(radians)? / std::f32::consts::TAU
    } else if let Some(grad) = angle.strip_suffix("grad") {
        number(grad)? / 400.
    } else {
        number(angle.strip_suffix("deg").unwrap_or(&angle))? / 360.
    };
    let saturation = percentage(channels[1])?.clamp(0., 1.);
    let lightness = percentage(channels[2])?.clamp(0., 1.);
    let chroma = (1. - (2. * lightness - 1.).abs()) * saturation;
    Some(Rgba::from_chroma(
        hue,
        chroma,
        lightness - chroma / 2.,
        alpha,
    ))
}

static LITERALS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)#[0-9a-f]{3,8}|(?:rgba?|hsla?)\([0-9a-z.%,\t /+\-]{1,96}\)")
        .expect("valid color literal pattern")
});

pub(crate) fn literals(line: &str) -> impl Iterator<Item = (Range<usize>, Rgba)> {
    let word = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '#');
    LITERALS.find_iter(line).filter_map(move |found| {
        let range = found.range();
        if line[..range.start].ends_with(word)
            || (found.as_str().starts_with('#') && line[range.end..].starts_with(word))
        {
            return None;
        }
        let color = if found.as_str().starts_with('#') {
            Rgba::hex(found.as_str())
        } else {
            functional(found.as_str())
        }?;
        Some((range, color))
    })
}

#[cfg(test)]
mod tests;

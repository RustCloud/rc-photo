//! Exposure settings from the flat tags of `use-ffmpeg.get-metadata`:
//! `exif.ISOSpeedRatings` → `200`, `exif.FNumber` → `18/10 (1.8000)`.

/// What the camera was set to.
#[derive(Debug, Default, PartialEq)]
pub struct Exposure {
    pub iso: Option<u32>,
    pub aperture: Option<f32>,
    pub exposure: Option<String>,
    pub focal: Option<f32>,
}

pub fn exposure(tags: &[(String, String)]) -> Exposure {
    let tag = |name: &str| tags.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str());
    Exposure {
        iso: tag("exif.ISOSpeedRatings").and_then(|v| v.trim().parse().ok()).filter(|&iso| iso > 0),
        aperture: tag("exif.FNumber").and_then(number).filter(|&f| f > 0.0).map(|f| round(f, 1)),
        exposure: tag("exif.ExposureTime").and_then(number).filter(|&t| t > 0.0).map(shutter),
        focal: tag("exif.FocalLength").and_then(number).filter(|&f| f > 0.0).map(|f| round(f, 1)),
    }
}

/// A number from `26`, `2.8` or a fraction `18/10 (1.8000)`.
fn number(value: &str) -> Option<f64> {
    let value = value.split_whitespace().next()?;
    match value.split_once('/') {
        Some((top, bottom)) => {
            let (top, bottom): (f64, f64) = (top.parse().ok()?, bottom.parse().ok()?);
            (bottom != 0.0).then(|| top / bottom)
        }
        None => value.parse().ok(),
    }
}

/// A shutter speed as photographers write it: `1/250`, `0.5s`, `2s`.
fn shutter(seconds: f64) -> String {
    if seconds < 0.4 {
        format!("1/{}", (1.0 / seconds).round())
    } else {
        let rounded = round(seconds, 1);
        if rounded.fract() == 0.0 {
            format!("{rounded:.0}s")
        } else {
            format!("{rounded}s")
        }
    }
}

fn round(value: f64, digits: i32) -> f32 {
    let scale = 10f64.powi(digits);
    ((value * scale).round() / scale) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn reads_what_nom_exif_writes() {
        let found = exposure(&tags(&[
            ("exif.ISOSpeedRatings", "200"),
            ("exif.FNumber", "18/10 (1.8000)"),
            ("exif.ExposureTime", "1/60 (0.0167)"),
            ("exif.FocalLength", "4250/1000 (4.2500)"),
            ("exif.Make", "Apple"),
        ]));
        assert_eq!(
            found,
            Exposure { iso: Some(200), aperture: Some(1.8), exposure: Some("1/60".into()), focal: Some(4.3) }
        );
    }

    #[test]
    fn shutter_speeds() {
        assert_eq!(shutter(1.0 / 250.0), "1/250");
        assert_eq!(shutter(10.0 / 600.0), "1/60");
        assert_eq!(shutter(0.5), "0.5s");
        assert_eq!(shutter(2.0), "2s");
    }

    #[test]
    fn junk_is_skipped() {
        let found = exposure(&tags(&[
            ("exif.ISOSpeedRatings", "U16Array[0x64]"),
            ("exif.FNumber", "0/0 (NaN)"),
            ("exif.ExposureTime", "abc"),
            ("exif.FocalLength", "0/1 (0.0000)"),
        ]));
        assert_eq!(found, Exposure::default());
        assert_eq!(exposure(&[]), Exposure::default());
    }
}

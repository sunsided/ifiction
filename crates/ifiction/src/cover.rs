//! Reads the format and size of a cover image from its header, without decoding it.

use crate::model::CoverFormat;

/// Why the image header could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverError {
    UnknownFormat,
    Malformed(&'static str),
}

impl std::fmt::Display for CoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFormat => {
                f.write_str("a cover is a PNG or JPEG image; this file is neither")
            }
            Self::Malformed(kind) => write!(f, "the {kind} header is truncated or malformed"),
        }
    }
}

impl std::error::Error for CoverError {}

/// The format, width and height of a PNG or JPEG image.
pub fn probe(bytes: &[u8]) -> Result<(CoverFormat, u32, u32), CoverError> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        // The IHDR chunk comes first: length, "IHDR", width, height.
        let ihdr = bytes.get(8..24).ok_or(CoverError::Malformed("PNG"))?;
        if &ihdr[4..8] != b"IHDR" {
            return Err(CoverError::Malformed("PNG"));
        }
        let w = u32::from_be_bytes([ihdr[8], ihdr[9], ihdr[10], ihdr[11]]);
        let h = u32::from_be_bytes([ihdr[12], ihdr[13], ihdr[14], ihdr[15]]);
        return Ok((CoverFormat::Png, w, h));
    }
    if bytes.starts_with(&[0xff, 0xd8]) {
        return jpeg_size(bytes).map(|(w, h)| (CoverFormat::Jpg, w, h));
    }
    Err(CoverError::UnknownFormat)
}

fn jpeg_size(bytes: &[u8]) -> Result<(u32, u32), CoverError> {
    let bad = CoverError::Malformed("JPEG");
    let mut i = 2;
    loop {
        // Skip fill bytes before a marker.
        while bytes.get(i) == Some(&0xff) && bytes.get(i + 1) == Some(&0xff) {
            i += 1;
        }
        let (&ff, &marker) = (
            bytes.get(i).ok_or(bad.clone())?,
            bytes.get(i + 1).ok_or(bad.clone())?,
        );
        if ff != 0xff {
            return Err(bad);
        }
        i += 2;
        if (0xd0..=0xd9).contains(&marker) || marker == 0x01 {
            continue;
        }
        let seg = bytes.get(i..i + 2).ok_or(bad.clone())?;
        let len = usize::from(u16::from_be_bytes([seg[0], seg[1]]));
        // Start-of-frame markers, except DHT (C4), JPG (C8) and DAC (CC).
        if matches!(marker, 0xc0..=0xcf) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            let sof = bytes.get(i + 2..i + 7).ok_or(bad.clone())?;
            let h = u16::from_be_bytes([sof[1], sof[2]]);
            let w = u16::from_be_bytes([sof[3], sof[4]]);
            return Ok((u32::from(w), u32::from(h)));
        }
        i += len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(960u32.to_be_bytes());
        png.extend(480u32.to_be_bytes());
        assert_eq!(probe(&png), Ok((CoverFormat::Png, 960, 480)));
    }

    #[test]
    fn jpeg() {
        let mut jpg = vec![0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x00, 0x00];
        jpg.extend([0xff, 0xc0, 0x00, 0x11, 0x08, 0x01, 0x2c, 0x02, 0x58]);
        assert_eq!(probe(&jpg), Ok((CoverFormat::Jpg, 600, 300)));
    }

    #[test]
    fn neither() {
        assert_eq!(probe(b"GIF89a"), Err(CoverError::UnknownFormat));
        assert_eq!(
            probe(b"\x89PNG\r\n\x1a\n"),
            Err(CoverError::Malformed("PNG"))
        );
    }
}

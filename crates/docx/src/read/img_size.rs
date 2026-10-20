//! Pixel size of a picture stored in a file, read from its header alone. While reading we need a
//! handful of bytes from the start of a picture, not the whole picture decoded, so these formats
//! are recognised by hand instead of through the `image` crate.

/// Pixel size of encoded image bytes (PNG, JPEG, GIF, BMP or WebP), `(None)` for anything else.
pub(super) fn pix_size(bytes: &[u8]) -> Option<(u32, u32)> {
    png(bytes).or_else(|| jpeg(bytes)).or_else(|| gif(bytes)).or_else(|| bmp(bytes)).or_else(|| webp(bytes))
}

fn png(b: &[u8]) -> Option<(u32, u32)> {
    let sig: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if b.len() >= 24 && b[..8] == sig && &b[12..16] == b"IHDR" {
        return Some((u32_be(&b[16..20]), u32_be(&b[20..24])));
    }
    None
}

/// Walks the marker segments to a frame header (SOF0-3, 5-7, 9-11, 13-15).
fn jpeg(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let m = b[i + 1];
        if m == 0xFF {
            i += 1; // fill bytes
            continue;
        }
        // Stand-alone markers carry no length.
        if m == 0x01 || (0xD0..=0xD7).contains(&m) {
            i += 2;
            continue;
        }
        let len = (usize::from(b[i + 2]) << 8) | usize::from(b[i + 3]);
        if len < 2 {
            return None;
        }
        // A frame header starts with height (5..7) and width (7..9) after precision.
        if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) && i + 9 <= b.len() {
            let h = (u32::from(b[i + 5]) << 8) | u32::from(b[i + 6]);
            let w = (u32::from(b[i + 7]) << 8) | u32::from(b[i + 8]);
            return Some((w, h));
        }
        i = i.checked_add(2 + len)?;
    }
    None
}

fn gif(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() >= 10 && &b[..3] == b"GIF" {
        return Some((u32::from(u16_le(&b[6..8])), u32::from(u16_le(&b[8..10]))));
    }
    None
}

fn bmp(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() >= 26 && &b[..2] == b"BM" {
        let w = i32_le(&b[18..22]);
        let h = i32_le(&b[22..26]).checked_abs()?;
        return Some((w.max(0) as u32, h.max(0) as u32));
    }
    None
}

/// WebP's three payload chunks: `VP8 ` (lossy), `VP8L` (lossless) and `VP8X` (extended).
fn webp(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 20 || &b[..4] != b"RIFF" || &b[8..12] != b"WEBP" {
        return None;
    }
    match &b[12..16] {
        b"VP8 " if b.len() >= 30 && b[23] == 0x9D && b[24] == 0x01 && b[25] == 0x2A => {
            return Some((u32::from(u16_le(&b[26..28]) & 0x3FFF), u32::from(u16_le(&b[28..30]) & 0x3FFF)));
        }
        // The signature byte is followed by a little-endian u32: width-1 (14 bits),
        // then height-1 (14 bits).
        b"VP8L" if b.len() >= 25 && b[20] == 0x2F => {
            let v = u32_le(&b[21..25]);
            return Some(((v & 0x3FFF) + 1, ((v >> 14) & 0x3FFF) + 1));
        }
        b"VP8X" if b.len() >= 30 => return Some((u24_le(&b[24..27]) + 1, u24_le(&b[27..30]) + 1)),
        _ => {}
    }
    None
}

fn u32_be(b: &[u8]) -> u32 {
    u32::from_be_bytes(b.try_into().unwrap_or([0; 4]))
}

fn u16_le(b: &[u8]) -> u16 {
    u16::from_le_bytes(b.try_into().unwrap_or([0; 2]))
}

fn u32_le(b: &[u8]) -> u32 {
    u32::from_le_bytes(b.try_into().unwrap_or([0; 4]))
}

fn u24_le(b: &[u8]) -> u32 {
    u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16
}

fn i32_le(b: &[u8]) -> i32 {
    i32::from_le_bytes(b.try_into().unwrap_or([0; 4]))
}

#[cfg(test)]
mod tests {
    use super::pix_size;

    #[test]
    fn reads_headers_of_the_common_formats() {
        // PNG: signature, IHDR length and code, 3 x 2 pixels.
        let mut png = vec![0x89u8, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&3u32.to_be_bytes());
        png.extend_from_slice(&2u32.to_be_bytes());
        assert_eq!(pix_size(&png), Some((3, 2)));

        // JPEG: SOI, a non-frame clump, an SOF0 frame with 4 x 5 pixels, padding.
        let jpeg = [
            0xFFu8, 0xD8, 0xFF, 0xE0, 0x00, 0x02, 0x00, 0x00, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x05, 0x00, 0x04, 0x01, 0x00, 0xFF, 0xFF, 0xFF,
            0xD9,
        ];
        assert_eq!(pix_size(&jpeg), Some((4, 5)));

        // GIF: logical screen 11 x 7.
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&11u16.to_le_bytes());
        gif.extend_from_slice(&7u16.to_le_bytes());
        assert_eq!(pix_size(&gif), Some((11, 7)));

        // BMP: BITMAPINFOHEADER with height stored positive and negative (bottom-up rows).
        let new_bmp = |h: i32| {
            let mut bmp = vec![0u8; 26];
            bmp[..2].copy_from_slice(b"BM");
            bmp[18..22].copy_from_slice(&6i32.to_le_bytes());
            bmp[22..26].copy_from_slice(&h.to_le_bytes());
            bmp
        };
        assert_eq!(pix_size(&new_bmp(9)), Some((6, 9)));
        assert_eq!(pix_size(&new_bmp(-9)), Some((6, 9)));

        // WebP, each payload: lossy sync, lossless bitstream, extended dimensions-1.
        let mut lossy = Vec::new();
        lossy.extend_from_slice(b"RIFF\x00\x00\x00\x00WEBPVP8 ");
        lossy.extend(12u32.to_le_bytes());
        lossy.extend_from_slice(&[0, 0, 0, 0x9D, 0x01, 0x2A, 5u8, 0, 6, 0]);
        assert_eq!(pix_size(&lossy), Some((5, 6)));

        let mut lossless = Vec::new();
        lossless.extend_from_slice(b"RIFF\x00\x00\x00\x00WEBPVP8L");
        lossless.extend(8u32.to_le_bytes());
        lossless.push(0x2F);
        let v: u32 = (10u32 - 1) | ((4u32 - 1) << 14);
        lossless.extend(v.to_le_bytes());
        assert_eq!(pix_size(&lossless), Some((10, 4)));

        let mut extended = Vec::new();
        extended.extend_from_slice(b"RIFF\x00\x00\x00\x00WEBPVP8X");
        extended.extend(16u32.to_le_bytes());
        extended.extend(0u32.to_le_bytes());
        extended.extend([7u8, 0, 0]);
        extended.extend([8u8, 0, 0]);
        assert_eq!(pix_size(&extended), Some((8, 9)));

        // Nothing readable, truncated frames.
        assert_eq!(pix_size(b""), None);
        assert_eq!(pix_size(b"\x89PNG\r\n\x1a\nxx"), None);
        assert_eq!(pix_size(&[0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x01]), None);
    }
}

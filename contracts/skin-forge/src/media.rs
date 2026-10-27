//! Collection icon and the canonical per-token preview.
//!
//! The preview is a 16×16 PNG derived from the paint and the screen. `media_hash`
//! is the SHA-256 of those exact bytes. A host that serves `{token_id}.png` has to
//! serve this file, or the hash will not match.

use fe2vi_core::SkinTraits;

const PAINTS: [[u8; 3]; 12] = [
    [0x9a, 0xa0, 0xa6],
    [0x7a, 0x4f, 0xc4],
    [0x1d, 0x2a, 0x3a],
    [0xe7, 0xe4, 0xdc],
    [0xe0, 0x74, 0x2a],
    [0x6b, 0x7a, 0x3a],
    [0xc4, 0x5c, 0x26],
    [0x1e, 0xc8, 0xc0],
    [0xd4, 0xa0, 0x17],
    [0xd5, 0xd8, 0xde],
    [0x14, 0x14, 0x16],
    [0x4a, 0x5a, 0x3a],
];

const SCREENS: [[u8; 3]; 8] = [
    [0xff, 0xc9, 0x28],
    [0xe0, 0x4b, 0xff],
    [0x2f, 0xe6, 0xe6],
    [0xc9, 0x4a, 0x1a],
    [0x2f, 0xb8, 0x6a],
    [0x9c, 0xff, 0x3a],
    [0xd4, 0xd2, 0xcc],
    [0xff, 0x4d, 0x6a],
];

pub const ICON_PNG: &[u8] = include_bytes!("../../../apps/web/public/brand/fe2vi-nav@1x.png");

pub fn trait_rows(traits: &SkinTraits) -> [(&'static str, &'static str); 9] {
    [
        ("Paint", at(&PAINT_NAMES, traits.paint)),
        ("Screen", at(&SCREEN_NAMES, traits.screen)),
        ("Eyes", at(&EYE_NAMES, traits.eyes)),
        ("Headwear", at(&HEADWEAR_NAMES, traits.headwear)),
        ("Face", at(&FACE_NAMES, traits.face_item)),
        ("Back", at(&BACK_NAMES, traits.back_item)),
        ("Emblem", at(&EMBLEM_NAMES, traits.emblem)),
        ("Trail", at(&TRAIL_NAMES, traits.trail)),
        ("Role", at(&ROLE_NAMES, traits.role)),
    ]
}

fn at(names: &'static [&'static str], index: u8) -> &'static str {
    names.get(usize::from(index)).copied().unwrap_or("Unknown")
}

const PAINT_NAMES: [&str; 12] = [
    "Steel",
    "Violet",
    "Night",
    "Paper",
    "Copper",
    "Olive",
    "Rust",
    "Teal",
    "Gold",
    "Chrome",
    "Matte black",
    "Camo",
];
const SCREEN_NAMES: [&str; 8] = [
    "Amber", "Magenta", "Cyan", "Signal", "Mint", "Lime", "White", "Rose",
];
const EYE_NAMES: [&str; 6] = ["Round", "Pill", "Visor", "Heart", "Sleepy", "Pixel"];
const HEADWEAR_NAMES: [&str; 8] = [
    "Beanie", "Headset", "Hood", "Hard hat", "Goggles", "Antenna", "Crown", "Cap",
];
const FACE_NAMES: [&str; 4] = ["Brow bar", "Monocle", "Mask", "Mic"];
const BACK_NAMES: [&str; 4] = ["Pack", "Jetpack", "Cape", "Array"];
const EMBLEM_NAMES: [&str; 6] = ["Key", "Coin", "Glyph", "Shield", "Badge", "Compass"];
const TRAIL_NAMES: [&str; 4] = ["Sparks", "Coins", "Glitch", "Smoke"];
const ROLE_NAMES: [&str; 6] = [
    "Safecracker",
    "Trader",
    "Hacker",
    "Auditor",
    "Builder",
    "Scout",
];

pub fn collection_icon() -> String {
    format!("data:image/png;base64,{}", base64_encode(ICON_PNG))
}

/// Paint fills the tile. The screen color is the face block.
pub fn preview_png(traits: &SkinTraits) -> Vec<u8> {
    let paint = color(&PAINTS, traits.paint);
    let screen = color(&SCREENS, traits.screen);
    let mut pixels = Vec::with_capacity(16 * 16 * 3);
    for y in 0..16u8 {
        for x in 0..16u8 {
            let face = (4..12).contains(&y) && (5..11).contains(&x);
            let rgb = if face { screen } else { paint };
            pixels.extend(rgb);
        }
    }
    png_rgb(16, 16, &pixels)
}

fn color(table: &[[u8; 3]], index: u8) -> [u8; 3] {
    table
        .get(usize::from(index))
        .copied()
        .unwrap_or([0x14, 0x14, 0x16])
}

pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= data.len() {
        let n = (u32::from(data[i]) << 16) | (u32::from(data[i + 1]) << 8) | u32::from(data[i + 2]);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        i += 3;
    }
    let rest = data.len() - i;
    if rest == 1 {
        let n = u32::from(data[i]) << 16;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let n = (u32::from(data[i]) << 16) | (u32::from(data[i + 1]) << 8);
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn png_rgb(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let mut scan = Vec::with_capacity((width as usize * 3 + 1) * height as usize);
    let row = (width as usize) * 3;
    for y in 0..height as usize {
        scan.push(0);
        scan.extend(&rgb[y * row..(y + 1) * row]);
    }
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]);
    let mut png = vec![0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
    png.extend(chunk(b"IHDR", &ihdr));
    png.extend(chunk(b"IDAT", &zlib_store(&scan)));
    png.extend(chunk(b"IEND", &[]));
    png
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + data.len());
    out.extend((data.len() as u32).to_be_bytes());
    out.extend(kind);
    out.extend(data);
    let mut crc_src = Vec::with_capacity(4 + data.len());
    crc_src.extend(kind);
    crc_src.extend(data);
    out.extend(crc32(&crc_src).to_be_bytes());
    out
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut offset = 0;
    while offset < data.len() || data.is_empty() && offset == 0 {
        let end = (offset + 65_535).min(data.len());
        let last = end == data.len();
        out.push(u8::from(last));
        let len = (end - offset) as u16;
        out.extend(len.to_le_bytes());
        out.extend((!len).to_le_bytes());
        out.extend(&data[offset..end]);
        offset = end;
        if data.is_empty() {
            break;
        }
    }
    out.extend(adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_is_a_png_and_the_icon_is_the_cut_out_mark() {
        let png = preview_png(&fe2vi_core::default_skin("scout.vault.fe2vi.testnet"));
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&ICON_PNG[..8], b"\x89PNG\r\n\x1a\n");
        assert!(collection_icon().starts_with("data:image/png;base64,"));
        assert_eq!(
            preview_png(&fe2vi_core::default_skin("scout.vault.fe2vi.testnet")),
            png
        );
    }
}

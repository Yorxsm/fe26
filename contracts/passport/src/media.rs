//! Compact passport card. The bytes are a function of the token id, so a mint
//! and a later recorder write produce the same image.

const PAINT: [&str; 12] = [
    "#9aa0a6", "#7a4fc4", "#1d2a3a", "#e7e4dc", "#e0742a", "#6b7a3a", "#c45c26", "#1ec8c0",
    "#d4a017", "#d5d8de", "#141416", "#4a5a3a",
];
const SCREEN: [&str; 8] = [
    "#ffc928", "#e04bff", "#2fe6e6", "#c94a1a", "#2fb86a", "#9cff3a", "#d4d2cc", "#ff4d6a",
];

/// `data:image/svg+xml;base64,…` for `token_id`. Same FNV mix as the site skin.
pub fn token_media(token_id: &str) -> String {
    let acc = fnv(token_id);
    let paint = PAINT[(acc % 12) as usize];
    let screen = SCREEN[((acc >> 4) % 8) as usize];
    let eyes = eyes((acc >> 7) % 6, screen);
    let hat = hat((acc >> 10) % 8, paint);
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 48 64'><rect width='48' height='64' fill='#0a0a0b'/><rect x='3' y='3' width='42' height='58' fill='none' stroke='{screen}'/><rect x='14' y='22' width='20' height='28' rx='3' fill='{paint}'/><rect x='17' y='26' width='14' height='10' fill='#0a0a0b'/>{eyes}<rect x='18' y='42' width='12' height='2' fill='{screen}'/>{hat}</svg>"
    );
    format!("data:image/svg+xml;base64,{}", b64(svg.as_bytes()))
}

pub fn b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let mut index = 0;
    while index + 3 <= data.len() {
        let n = (u32::from(data[index]) << 16)
            | (u32::from(data[index + 1]) << 8)
            | u32::from(data[index + 2]);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push(T[(n & 63) as usize] as char);
        index += 3;
    }
    let rest = data.len() - index;
    if rest == 1 {
        let n = u32::from(data[index]) << 16;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rest == 2 {
        let n = (u32::from(data[index]) << 16) | (u32::from(data[index + 1]) << 8);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(T[((n >> 6) & 63) as usize] as char);
        out.push('=');
    }
    out
}

fn fnv(token_id: &str) -> u32 {
    let mut acc: u32 = 2_166_136_261;
    for byte in token_id.as_bytes() {
        acc ^= u32::from(*byte);
        acc = acc.wrapping_mul(16_777_619);
    }
    acc
}

fn eyes(style: u32, color: &str) -> String {
    match style {
        1 => format!("<rect x='18' y='29' width='4' height='2' fill='{color}'/><rect x='26' y='29' width='4' height='2' fill='{color}'/>"),
        2 => format!("<rect x='18' y='29' width='12' height='2' fill='{color}'/>"),
        3 => format!("<circle cx='20' cy='30' r='2' fill='{color}'/><circle cx='28' cy='30' r='2' fill='{color}'/>"),
        4 => format!("<path d='M18 31h4M26 31h4' stroke='{color}'/>"),
        5 => format!("<rect x='19' y='28' width='2' height='2' fill='{color}'/><rect x='27' y='28' width='2' height='2' fill='{color}'/>"),
        _ => format!("<circle cx='20' cy='30' r='1.5' fill='{color}'/><circle cx='28' cy='30' r='1.5' fill='{color}'/>"),
    }
}

fn hat(style: u32, paint: &str) -> String {
    match style {
        0 => format!("<rect x='16' y='12' width='16' height='6' fill='{paint}'/>"),
        1 => "<rect x='10' y='24' width='3' height='8' fill='#cfcfcf'/><rect x='35' y='24' width='3' height='8' fill='#cfcfcf'/>".to_string(),
        2 => format!("<path d='M14 20 24 10 34 20' fill='{paint}'/>"),
        3 => "<rect x='12' y='14' width='24' height='4' fill='#d4a017'/>".to_string(),
        4 => "<rect x='16' y='28' width='16' height='3' fill='#222'/>".to_string(),
        5 => format!("<rect x='23' y='12' width='2' height='8' fill='{paint}'/><circle cx='24' cy='11' r='2' fill='{paint}'/>"),
        6 => "<path d='M16 18 24 8 32 18' fill='#d4a017'/>".to_string(),
        _ => format!("<rect x='14' y='14' width='20' height='4' fill='{paint}'/>"),
    }
}

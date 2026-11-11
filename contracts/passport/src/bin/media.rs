use sha2::Digest;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(token_id) = args.next() else {
        eprintln!("usage: passport-media <token_id>");
        std::process::exit(1);
    };
    if args.next().is_some() || !valid_token(&token_id) {
        eprintln!("usage: passport-media <token_id>");
        std::process::exit(1);
    }
    let media = passport::token_media(&token_id);
    let digest = sha2::Sha256::digest(media.as_bytes());
    println!("{media}");
    println!("{}", passport::b64(&digest));
}

fn valid_token(token_id: &str) -> bool {
    let bytes = token_id.as_bytes();
    (3..=64).contains(&bytes.len())
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_' || *byte == b'-')
}

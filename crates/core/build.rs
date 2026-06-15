//! Inline KaTeX's woff2 fonts into its stylesheet as `data:` URIs at build
//! time, producing a single self-contained CSS string. The vendored CSS lists
//! woff2 first, so browsers use the embedded data URI and never request the
//! woff/ttf fallbacks — rendered pages need no network and no sidecar assets.

use std::env;
use std::fs;
use std::path::Path;

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        s.push(T[(n >> 18 & 63) as usize] as char);
        s.push(T[(n >> 12 & 63) as usize] as char);
        s.push(if chunk.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        s.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    s
}

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let css_path = Path::new(&manifest).join("assets/katex/katex.min.css");
    let fonts_dir = Path::new(&manifest).join("assets/katex/fonts");
    println!("cargo:rerun-if-changed={}", css_path.display());
    println!("cargo:rerun-if-changed={}", fonts_dir.display());

    let mut css = fs::read_to_string(&css_path).expect("vendored katex.min.css");
    for entry in fs::read_dir(&fonts_dir).expect("vendored katex fonts") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("woff2") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = fs::read(&path).unwrap();
        let data_uri = format!("url(data:font/woff2;base64,{})", base64(&bytes));
        css = css.replace(&format!("url(fonts/{name})"), &data_uri);
    }

    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("katex-inlined.css");
    fs::write(&out, css).unwrap();
}

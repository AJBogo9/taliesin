//! A minimal ZIP archive writer (store + deflate), just enough to package a built book
//! into one downloadable file for the "read offline" affordance.
//!
//! `flate2` + `crc32fast` are already in the dependency graph transitively, so this adds no
//! new crate — and a hand-rolled container keeps the `zip` crate (and its transitive
//! fan-out: bzip2/zstd/time/…) out of the tree, in keeping with the project's
//! dependency-minimalism. Only the classic 32-bit format is emitted (no zip64), which is
//! correct for a book: well under the 4 GiB / 65 535-entry limits.

use crc32fast::Hasher;
use flate2::Compression;
use flate2::write::DeflateEncoder;
use std::io::Write;

/// A file to archive: a forward-slashed archive path plus its bytes.
pub struct ZipEntry {
    pub name: String,
    pub data: Vec<u8>,
}

// The four ZIP signatures, DOS epoch date (1980-01-01), and the UTF-8-filename flag.
const LOCAL_SIG: u32 = 0x0403_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const EOCD_SIG: u32 = 0x0605_4b50;
const DOS_DATE_1980: u16 = 0x0021;
const FLAG_UTF8: u16 = 0x0800;

/// Build a ZIP archive in memory from `entries`. Each entry is deflated when that shrinks
/// it (method 8), else stored uncompressed (method 0) — so already-compressed images never
/// grow. Names are written as UTF-8 (the general-purpose bit-11 flag is set).
pub fn build_zip(entries: &[ZipEntry]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    // Per entry, the fields the central directory needs: (name, crc, comp_size, size, method, offset).
    let mut central: Vec<(&str, u32, u32, u32, u16, u32)> = Vec::with_capacity(entries.len());

    for entry in entries {
        let data = &entry.data;
        let mut hasher = Hasher::new();
        hasher.update(data);
        let crc = hasher.finalize();

        // Raw DEFLATE (RFC 1951, no zlib wrapper) is exactly what ZIP method 8 stores.
        let deflated = {
            let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
            let _ = e.write_all(data);
            e.finish().unwrap_or_default()
        };
        let (method, payload): (u16, &[u8]) = if !deflated.is_empty() && deflated.len() < data.len()
        {
            (8, &deflated)
        } else {
            (0, data)
        };

        let offset = out.len() as u32;
        let name = entry.name.as_bytes();

        out.extend_from_slice(&LOCAL_SIG.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&FLAG_UTF8.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // mod time
        out.extend_from_slice(&DOS_DATE_1980.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(name);
        out.extend_from_slice(payload);

        central.push((
            &entry.name,
            crc,
            payload.len() as u32,
            data.len() as u32,
            method,
            offset,
        ));
    }

    let cd_start = out.len() as u32;
    for (name, crc, comp, size, method, offset) in &central {
        let name = name.as_bytes();
        out.extend_from_slice(&CENTRAL_SIG.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version made by
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&FLAG_UTF8.to_le_bytes());
        out.extend_from_slice(&method.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // mod time
        out.extend_from_slice(&DOS_DATE_1980.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&comp.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra len
        out.extend_from_slice(&0u16.to_le_bytes()); // comment len
        out.extend_from_slice(&0u16.to_le_bytes()); // disk number start
        out.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        out.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        out.extend_from_slice(offset.to_le_bytes().as_slice());
        out.extend_from_slice(name);
    }
    let cd_size = out.len() as u32 - cd_start;

    let count = central.len() as u16;
    out.extend_from_slice(&EOCD_SIG.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // this disk number
    out.extend_from_slice(&0u16.to_le_bytes()); // disk with central dir start
    out.extend_from_slice(&count.to_le_bytes()); // entries on this disk
    out.extend_from_slice(&count.to_le_bytes()); // entries total
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment len
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_archive_is_a_bare_eocd() {
        let z = build_zip(&[]);
        // Just the 22-byte end-of-central-directory record with zero entries.
        assert_eq!(z.len(), 22);
        assert_eq!(&z[..4], &EOCD_SIG.to_le_bytes());
    }

    #[test]
    fn stores_incompressible_and_deflates_compressible() {
        // A short, high-entropy blob won't shrink under deflate → stored (method 0). A long
        // repetitive blob will → deflated (method 8). Method lives at offset 8 of each local
        // header, and the archive starts at a local header.
        let incompressible = ZipEntry {
            name: "a.bin".into(),
            data: vec![0, 1, 2, 3],
        };
        let compressible = ZipEntry {
            name: "b.txt".into(),
            data: vec![b'x'; 4096],
        };
        let z = build_zip(&[incompressible]);
        assert_eq!(
            u16::from_le_bytes([z[8], z[9]]),
            0,
            "tiny blob must be stored"
        );
        let z = build_zip(&[compressible]);
        assert_eq!(
            u16::from_le_bytes([z[8], z[9]]),
            8,
            "repetitive blob must deflate"
        );
    }

    #[test]
    fn eocd_entry_count_matches() {
        let z = build_zip(&[
            ZipEntry {
                name: "one.txt".into(),
                data: b"hello".to_vec(),
            },
            ZipEntry {
                name: "two.txt".into(),
                data: b"world".to_vec(),
            },
        ]);
        // EOCD is the last 22 bytes; entry count is the u16 at EOCD+10.
        let eocd = &z[z.len() - 22..];
        assert_eq!(&eocd[..4], &EOCD_SIG.to_le_bytes());
        assert_eq!(u16::from_le_bytes([eocd[10], eocd[11]]), 2);
    }
}

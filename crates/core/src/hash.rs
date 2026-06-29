//! The one content-hashing primitive, shared by the block-id scheme (`render`) and the
//! freeze cache-key scheme (`qmd-fast-server`'s `freeze`). A block's content-hash id and
//! its execution-cache key are the **same** scheme, so they must hash identically — kept
//! here as a single definition rather than byte-identical copies that could silently drift.
//!
//! Do NOT swap the algorithm: seahash reintroduces cross-version instability that would
//! break content-hash block ids across tool versions, and xxhash/blake3 solve a non-problem
//! here (this is a short-string, non-adversarial digest, not a checksum).

/// 64-bit FNV-1a hash of `s` — small, deterministic, and stable across runs and tool
/// versions. The offset basis and prime are the canonical FNV-1a-64 constants.
pub fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_pins_the_offset_basis_and_is_deterministic() {
        // The empty string mixes no bytes, so it hashes to the FNV-1a-64 offset basis —
        // pinning the constant. (The corpus block-id stability tests pin real end-to-end
        // outputs; this guards the primitive itself.) Determinism + input-sensitivity
        // round out the contract the block-id == cache-key identity relies on.
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a("qmd-fast"), fnv1a("qmd-fast"));
        assert_ne!(fnv1a("a"), fnv1a("b"));
    }
}

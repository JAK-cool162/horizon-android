//! libsodium's sealed box (`crypto_box_seal`), the encryption GitHub's secrets API takes: an
//! ephemeral X25519 key, XSalsa20-Poly1305 and a BLAKE2b nonce, on crates the workspace already
//! locks. No network or JNI, so the host content tests cover it (`tools/test_android_content.py`).
use curve25519_dalek::montgomery::MontgomeryPoint;
use poly1305::{Poly1305, universal_hash::KeyInit};
use salsa20::{
    XSalsa20,
    cipher::{KeyIvInit, StreamCipher, consts::U10, generic_array::GenericArray},
};

/// `message` sealed for `recipient` (an X25519 public key) with a fresh ephemeral key.
pub fn seal(recipient: &[u8; 32], message: &[u8]) -> Result<Vec<u8>, String> {
    let mut secret = [0u8; 32];
    getrandom::fill(&mut secret).map_err(|e| e.to_string())?;
    Ok(seal_with(&secret, recipient, message))
}

/// [`seal`] with a given ephemeral secret: `ephemeral public || tag || ciphertext`.
pub fn seal_with(secret: &[u8; 32], recipient: &[u8; 32], message: &[u8]) -> Vec<u8> {
    let ephemeral = MontgomeryPoint::mul_base_clamped(*secret).to_bytes();
    let shared = MontgomeryPoint(*recipient).mul_clamped(*secret).to_bytes();
    // crypto_box_beforenm: HSalsa20 of the shared point, with a zero input.
    let key = salsa20::hsalsa::<U10>(GenericArray::from_slice(&shared), &GenericArray::default());
    let mut both = [0u8; 64];
    both[..32].copy_from_slice(&ephemeral);
    both[32..].copy_from_slice(recipient);
    let nonce = blake2b::<24>(&both);
    // crypto_secretbox: the stream's first 32 bytes key Poly1305, the rest encrypts.
    let mut cipher = XSalsa20::new(&key, GenericArray::from_slice(&nonce));
    let mut one_time = [0u8; 32];
    cipher.apply_keystream(&mut one_time);
    let mut body = message.to_vec();
    cipher.apply_keystream(&mut body);
    let tag = Poly1305::new(GenericArray::from_slice(&one_time)).compute_unpadded(&body);
    let mut out = Vec::with_capacity(48 + body.len());
    out.extend_from_slice(&ephemeral);
    out.extend_from_slice(&tag);
    out.extend_from_slice(&body);
    out
}

/// Unkeyed BLAKE2b with an `N`-byte digest (RFC 7693).
pub fn blake2b<const N: usize>(data: &[u8]) -> [u8; N] {
    const IV: [u64; 8] = [
        0x6a09e667f3bcc908,
        0xbb67ae8584caa73b,
        0x3c6ef372fe94f82b,
        0xa54ff53a5f1d36f1,
        0x510e527fade682d1,
        0x9b05688c2b3e6c1f,
        0x1f83d9abfb41bd6b,
        0x5be0cd19137e2179,
    ];
    const SIGMA: [[usize; 16]; 12] = [
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
        [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
        [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
        [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
        [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
        [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
        [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
        [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
        [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    ];
    assert!((1..=64).contains(&N));
    let mut h = IV;
    h[0] ^= 0x0101_0000 ^ N as u64;
    let compress = |h: &mut [u64; 8], block: &[u8; 128], counter: u128, last: bool| {
        let m: [u64; 16] = std::array::from_fn(|i| {
            u64::from_le_bytes(block[i * 8..i * 8 + 8].try_into().unwrap())
        });
        let mut v = [0u64; 16];
        v[..8].copy_from_slice(h);
        v[8..].copy_from_slice(&IV);
        v[12] ^= counter as u64;
        v[13] ^= (counter >> 64) as u64;
        if last {
            v[14] = !v[14];
        }
        for s in &SIGMA {
            let mut g = |a: usize, b: usize, c: usize, d: usize, x: u64, y: u64| {
                v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
                v[d] = (v[d] ^ v[a]).rotate_right(32);
                v[c] = v[c].wrapping_add(v[d]);
                v[b] = (v[b] ^ v[c]).rotate_right(24);
                v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
                v[d] = (v[d] ^ v[a]).rotate_right(16);
                v[c] = v[c].wrapping_add(v[d]);
                v[b] = (v[b] ^ v[c]).rotate_right(63);
            };
            g(0, 4, 8, 12, m[s[0]], m[s[1]]);
            g(1, 5, 9, 13, m[s[2]], m[s[3]]);
            g(2, 6, 10, 14, m[s[4]], m[s[5]]);
            g(3, 7, 11, 15, m[s[6]], m[s[7]]);
            g(0, 5, 10, 15, m[s[8]], m[s[9]]);
            g(1, 6, 11, 12, m[s[10]], m[s[11]]);
            g(2, 7, 8, 13, m[s[12]], m[s[13]]);
            g(3, 4, 9, 14, m[s[14]], m[s[15]]);
        }
        for i in 0..8 {
            h[i] ^= v[i] ^ v[i + 8];
        }
    };
    let mut counter = 0u128;
    let mut chunks = data.chunks(128).peekable();
    if chunks.peek().is_none() {
        compress(&mut h, &[0u8; 128], 0, true);
    }
    while let Some(chunk) = chunks.next() {
        let mut block = [0u8; 128];
        block[..chunk.len()].copy_from_slice(chunk);
        counter += chunk.len() as u128;
        compress(&mut h, &block, counter, chunks.peek().is_none());
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = h[i / 8].to_le_bytes()[i % 8];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
    #[test]
    fn blake2b_matches_rfc_7693_and_short_digests() {
        // RFC 7693 appendix A, and Python's hashlib.blake2b for the other lengths.
        assert_eq!(
            hex(&blake2b::<64>(b"abc")),
            "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d1\
             7d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923"
        );
        assert_eq!(
            hex(&blake2b::<24>(b"abc")),
            "56a17e38cc371a46b12c32f18e0c61de2a84e9c2555b114e"
        );
        let long: Vec<u8> = (0..300u32).map(|i| i as u8).collect();
        assert_eq!(blake2b::<24>(&long).len(), 24);
    }
    #[test]
    fn a_sealed_box_matches_an_independent_implementation() {
        // From a separate Python implementation (OpenSSL's X25519 and Poly1305, hashlib's
        // BLAKE2b, Salsa20 from its specification), which also opened it with the private key.
        let recipient: [u8; 32] = (1..=32u8)
            .map(|b| b)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let recipient = MontgomeryPoint::mul_base_clamped(recipient).to_bytes();
        assert_eq!(
            hex(&recipient),
            "07a37cbc142093c8b755dc1b10e86cb426374ad16aa853ed0bdfc0b2b86d1c7c"
        );
        let secret: [u8; 32] = (101..133u8).collect::<Vec<_>>().try_into().unwrap();
        let sealed = seal_with(
            &secret,
            &recipient,
            b"github_pat_example-token-for-a-sealed-box-test",
        );
        assert_eq!(
            hex(&sealed),
            "5714769d116bf76436ae74bc793d2c30ad1903c59ac5273805c7e2698b410c36\
             b2adf8848535210b4c409e80af65d869\
             46f7570a7667c3767b8864d6b116d3839a08d6e7e9f8e6c2a2fa7fa3aaccf58cd6b15b2da93cfc318bd3b56da134"
        );
        let fresh = seal(&recipient, b"x").unwrap();
        assert_eq!(fresh.len(), 49);
        assert_ne!(fresh[..32], sealed[..32]);
    }
}

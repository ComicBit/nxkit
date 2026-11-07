//! AES-256 decryption (FIPS 197) in safe Rust, ECB over whole blocks: Unreal Engine pak indexes and files encrypted
//! with the game's key (`uepak.py` `aes_decrypt`, CommonCrypto there).

const fn xtime(a: u8) -> u8 {
    (a << 1) ^ (if a & 0x80 != 0 { 0x1b } else { 0 })
}

const fn gmul(mut a: u8, mut b: u8) -> u8 {
    let mut p = 0u8;
    while b != 0 {
        if b & 1 != 0 {
            p ^= a;
        }
        a = xtime(a);
        b >>= 1;
    }
    p
}

const fn sbox_table() -> [u8; 256] {
    let mut s = [0u8; 256];
    let mut x = 0usize;
    while x < 256 {
        let a = x as u8;
        let mut inv = 1u8;
        let mut base = a;
        let mut e = 254u32;
        while e != 0 {
            if e & 1 != 0 {
                inv = gmul(inv, base);
            }
            base = gmul(base, base);
            e >>= 1;
        }
        if a == 0 {
            inv = 0;
        }
        let b = inv;
        s[x] = b ^ b.rotate_left(1) ^ b.rotate_left(2) ^ b.rotate_left(3) ^ b.rotate_left(4) ^ 0x63;
        x += 1;
    }
    s
}

const SBOX: [u8; 256] = sbox_table();

const fn inv_sbox_table() -> [u8; 256] {
    let mut r = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        r[SBOX[i] as usize] = i as u8;
        i += 1;
    }
    r
}

const INV_SBOX: [u8; 256] = inv_sbox_table();

const fn mul_table(k: u8) -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = gmul(i as u8, k);
        i += 1;
    }
    t
}

const M9: [u8; 256] = mul_table(9);
const M11: [u8; 256] = mul_table(11);
const M13: [u8; 256] = mul_table(13);
const M14: [u8; 256] = mul_table(14);

/// An expanded AES-256 key (decryption only), zeroed on drop.
pub struct Aes256 {
    rk: [[u8; 16]; 15],
}

impl Drop for Aes256 {
    fn drop(&mut self) {
        self.rk = [[0; 16]; 15];
    }
}

impl Aes256 {
    /// Expands a 32-byte key.
    pub fn new(key: &[u8; 32]) -> Aes256 {
        let mut w = [[0u8; 4]; 60];
        for (i, x) in w.iter_mut().take(8).enumerate() {
            x.copy_from_slice(&key[4 * i..4 * i + 4]);
        }
        let mut rcon = 1u8;
        for i in 8..60 {
            let mut t = w[i - 1];
            if i % 8 == 0 {
                t = [
                    SBOX[t[1] as usize] ^ rcon,
                    SBOX[t[2] as usize],
                    SBOX[t[3] as usize],
                    SBOX[t[0] as usize],
                ];
                rcon = xtime(rcon);
            } else if i % 8 == 4 {
                t = t.map(|b| SBOX[b as usize]);
            }
            for k in 0..4 {
                w[i][k] = w[i - 8][k] ^ t[k];
            }
        }
        let rk = std::array::from_fn(|r| {
            let mut b = [0u8; 16];
            for c in 0..4 {
                b[4 * c..4 * c + 4].copy_from_slice(&w[4 * r + c]);
            }
            b
        });
        Aes256 { rk }
    }

    /// Decrypts one block in place.
    pub fn decrypt_block(&self, b: &mut [u8; 16]) {
        let add = |b: &mut [u8; 16], r: usize| {
            for (x, k) in b.iter_mut().zip(&self.rk[r]) {
                *x ^= k;
            }
        };
        add(b, 14);
        for r in (0..14).rev() {
            // inverse shift rows (row i moves right by i), then the inverse S-box
            let s = *b;
            for c in 0..4 {
                for i in 0..4 {
                    b[4 * ((c + i) % 4) + i] = INV_SBOX[s[4 * c + i] as usize];
                }
            }
            add(b, r);
            if r > 0 {
                for c in 0..4 {
                    let a = [b[4 * c], b[4 * c + 1], b[4 * c + 2], b[4 * c + 3]];
                    for i in 0..4 {
                        b[4 * c + i] = M14[a[i] as usize]
                            ^ M11[a[(i + 1) % 4] as usize]
                            ^ M13[a[(i + 2) % 4] as usize]
                            ^ M9[a[(i + 3) % 4] as usize];
                    }
                }
            }
        }
    }

    /// ECB decryption of the whole blocks of `data`, a tail shorter than a block is left as is.
    pub fn decrypt_ecb(&self, data: &mut [u8]) {
        for c in data.chunks_exact_mut(16) {
            let mut b = [0u8; 16];
            b.copy_from_slice(c);
            self.decrypt_block(&mut b);
            c.copy_from_slice(&b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
            .collect()
    }

    #[test]
    fn fips197_and_sp800_38a_vectors() {
        // FIPS 197 appendix C.3
        let key: [u8; 32] = std::array::from_fn(|i| i as u8);
        let mut b: [u8; 16] = hex("8ea2b7ca516745bfeafc49904b496089")
            .try_into()
            .unwrap_or_default();
        Aes256::new(&key).decrypt_block(&mut b);
        assert_eq!(b.to_vec(), hex("00112233445566778899aabbccddeeff"));
        // SP 800-38A F.1.6, ECB-AES256.Decrypt, the first two blocks
        let k: [u8; 32] = hex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4")
            .try_into()
            .unwrap_or([0; 32]);
        let mut d = hex("f3eed1bdb5d2a03c064b5a7e3db181f8591ccb10d410ed26dc5ba74a31362870");
        d.push(7);
        Aes256::new(&k).decrypt_ecb(&mut d);
        assert_eq!(
            d,
            [
                hex("6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51"),
                vec![7]
            ]
            .concat()
        );
    }
}

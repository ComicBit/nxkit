//! Shift JIS as Python's `bytes.decode('shift-jis', 'replace')` reads it (the names of BND3 entries): ASCII, half-width
//! katakana and the JIS X 0208 characters of two bytes. A byte that starts no character becomes U+FFFD and the next
//! byte is read again.

mod table;

#[cfg(test)]
mod tests;

/// The two-byte character of `lead` and `trail`, when the codec has one.
fn pair(lead: u8, trail: u8) -> Option<char> {
    if !matches!(lead, 0x81..=0x9f | 0xe0..=0xfc) || !matches!(trail, 0x40..=0x7e | 0x80..=0xfc) {
        return None;
    }
    let c1 = usize::from(if lead < 0xe0 {
        lead - 0x81
    } else {
        lead - 0xc1
    });
    let c2 = usize::from(if trail < 0x80 {
        trail - 0x40
    } else {
        trail - 0x41
    });
    let row = 2 * c1 + usize::from(c2 >= 0x5e);
    let col = if c2 < 0x5e { c2 } else { c2 - 0x5e };
    match table::JIS0208.get(row)?.get(col).copied()? {
        0 => None,
        u => char::from_u32(u32::from(u)),
    }
}

/// `b.decode('shift-jis', 'replace')`.
pub fn decode(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        i += 1;
        match c {
            0..=0x7f => s.push(char::from(c)),
            0xa1..=0xdf => {
                s.push(char::from_u32(0xff61 + u32::from(c) - 0xa1).unwrap_or('\u{fffd}'))
            }
            _ => match b.get(i).and_then(|&t| pair(c, t)) {
                Some(ch) => {
                    s.push(ch);
                    i += 1;
                }
                None => s.push('\u{fffd}'),
            },
        }
    }
    s
}

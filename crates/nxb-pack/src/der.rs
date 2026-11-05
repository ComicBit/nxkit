//! The DER form of entitlements (the blob of special slot 7, magic `0xfade7172`).
//!
//! Layout, as `codesign` writes it (derived from its output):
//! `[APPLICATION 16] { INTEGER 1, [CONTEXT 16] { SEQUENCE { UTF8String key, value } ... } }`.
//! Dictionary entries are sorted by key bytes, a nested dictionary is again `[CONTEXT 16]`.
//! arrays are `SEQUENCE`, strings `UTF8String`, booleans `BOOLEAN` (`0xff` for true).
//! integers minimal two's-complement `INTEGER`. Other value types are rejected.

use crate::error::{Error, Result};
use crate::plist::Value;

/// Encodes an entitlements dictionary.
pub fn encode_entitlements(v: &Value) -> Result<Vec<u8>> {
    let Value::Dict(_) = v else {
        return Err(Error::Format("entitlements are not a dictionary".into()));
    };
    let mut body = Vec::new();
    tlv(0x02, &[1], &mut body);
    encode_value(v, &mut body)?;
    let mut out = Vec::new();
    tlv(0x70, &body, &mut out);
    Ok(out)
}

fn encode_value(v: &Value, out: &mut Vec<u8>) -> Result<()> {
    match v {
        Value::Dict(entries) => {
            let mut sorted: Vec<&(String, Value)> = entries.iter().collect();
            sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            let mut body = Vec::new();
            for (k, v) in sorted {
                let mut pair = Vec::new();
                tlv(0x0c, k.as_bytes(), &mut pair);
                encode_value(v, &mut pair)?;
                tlv(0x30, &pair, &mut body);
            }
            tlv(0xb0, &body, out);
        }
        Value::Array(items) => {
            let mut body = Vec::new();
            for v in items {
                encode_value(v, &mut body)?;
            }
            tlv(0x30, &body, out);
        }
        Value::String(s) => tlv(0x0c, s.as_bytes(), out),
        Value::Bool(b) => tlv(0x01, &[if *b { 0xff } else { 0 }], out),
        Value::Integer(n) => tlv(0x02, &minimal_int(*n), out),
        other => {
            return Err(Error::Unsupported(format!(
                "entitlement value {other:?} in DER form"
            )));
        }
    }
    Ok(())
}

fn minimal_int(n: i64) -> Vec<u8> {
    let b = n.to_be_bytes();
    let mut i = 0;
    while i < 7 {
        let redundant =
            (b[i] == 0 && b[i + 1] & 0x80 == 0) || (b[i] == 0xff && b[i + 1] & 0x80 != 0);
        if !redundant {
            break;
        }
        i += 1;
    }
    b[i..].to_vec()
}

fn tlv(tag: u8, body: &[u8], out: &mut Vec<u8>) {
    out.push(tag);
    let n = body.len();
    if n < 0x80 {
        out.push(n as u8);
    } else {
        let bytes = (n as u64).to_be_bytes();
        let skip = bytes.iter().take_while(|b| **b == 0).count();
        out.push(0x80 | (8 - skip) as u8);
        out.extend_from_slice(&bytes[skip..]);
    }
    out.extend_from_slice(body);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::hex;

    #[test]
    fn matches_codesign_sample() {
        // Entitlements file and DER as written by codesign (macOS 27, see README).
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
	<key>com.apple.security.get-task-allow</key>
	<true/>
	<key>com.apple.developer.kernel.increased-memory-limit</key>
	<true/>
	<key>application-identifier</key>
	<string>ABC.org.x</string>
	<key>keychain-access-groups</key>
	<array><string>ABC.*</string></array>
	<key>n</key>
	<integer>5</integer>
</dict>
</plist>"#;
        let v = crate::plist::parse(xml).unwrap();
        let der = encode_entitlements(&v).unwrap();
        assert_eq!(
            hex(&der),
            "7081b6020101b081b030230c166170706c69636174696f6e2d6964656e7469666965720c094142432e6f72672e7830360c31636f6d2e6170706c652e646576656c6f7065722e6b65726e656c2e696e637265617365642d6d656d6f72792d6c696d69740101ff30260c21636f6d2e6170706c652e73656375726974792e6765742d7461736b2d616c6c6f770101ff30210c166b6579636861696e2d6163636573732d67726f75707330070c054142432e2a30060c016e020105"
        );
    }

    #[test]
    fn integers() {
        assert_eq!(minimal_int(0), vec![0]);
        assert_eq!(minimal_int(127), vec![0x7f]);
        assert_eq!(minimal_int(128), vec![0, 0x80]);
        assert_eq!(minimal_int(-1), vec![0xff]);
        assert_eq!(minimal_int(-129), vec![0xff, 0x7f]);
    }
}

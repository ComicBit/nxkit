//! Bitcode for the AIR text IR of `air-emit`: a parser of the text subset air-emit writes and an
//! LLVM 14 era bitcode writer (typed pointers, string table, unabbreviated records, no value
//! names). Apple's `air-opt` reads it directly, so the text parse and the separate
//! `--discard-value-names` call of the back half go away. Anything outside the subset is an
//! error, the caller then hands the text to Apple's tools for that shader.

#![deny(missing_docs)]

pub mod ast;
pub mod bits;
pub mod opt;
pub mod parse;
pub mod split;
pub mod writer;

/// Text IR to bitcode.
pub fn to_bitcode(text: &str) -> Result<Vec<u8>, String> {
    let m = parse::parse(text)?;
    writer::write(&m)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = r#"; ModuleID = '-'
source_filename = "-"
target datalayout = "e-p:64:64:64-i1:8:8-i8:8:8-i16:16:16-i32:32:32-i64:64:64-f32:32:32-f64:64:64-v16:16:16-v24:32:32-v32:32:32-v48:64:64-v64:64:64-v96:128:128-v128:128:128-v192:256:256-v256:256:256-v512:512:512-v1024:1024:1024-n8:16:32"
target triple = "air64-apple-macosx14.0.0"

define float @f(float noundef %a) #0 {
entry:
  %a.addr = alloca float, align 4
  store float %a, float* %a.addr, align 4
  %0 = load float, float* %a.addr, align 4
  %add = fadd fast float %0, 1.000000e+00
  ret float %add
}

attributes #0 = { nounwind }
"#;

    #[test]
    fn writes_wrapped_bitcode() {
        let b = to_bitcode(SMALL).expect("subset");
        // wrapper magic 0x0B17C0DE, then the raw 'BC' 0xC0DE stream at the offset it names
        assert_eq!(&b[..4], &[0xDE, 0xC0, 0x17, 0x0B]);
        let off = u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize;
        assert_eq!(&b[off..off + 4], b"BC\xC0\xDE");
        assert_eq!(b.len() % 4, 0);
    }

    #[test]
    fn parses_function() {
        let m = parse::parse(SMALL).expect("subset");
        assert_eq!(m.functions.len(), 1);
        assert_eq!(m.functions[0].blocks[0].1.len(), 5);
    }

    #[test]
    fn rejects_unknown_instruction() {
        let bad = SMALL.replace("fadd fast float %0, 1.000000e+00", "frobnicate float %0");
        assert!(to_bitcode(&bad).is_err());
    }
}

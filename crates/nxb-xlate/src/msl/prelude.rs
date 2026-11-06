//! The fixed text of a library: the header and the helper functions the body calls.

use super::abi;
use std::collections::BTreeSet;

/// The header of every library.
pub fn header(reduced_precision: bool) -> String {
    let mut s =
        String::from("#include <metal_stdlib>\n#include <simd/simd.h>\n\nusing namespace metal;\n");
    if !reduced_precision {
        s.push_str("#pragma clang fp contract(off)\n");
    }
    s
}

const FIND_MSB: &str = "
static inline __attribute__((always_inline)) uint dm_find_umsb(uint x) { return x == 0u ? 0xffffffffu : 31u - clz(x); }
static inline __attribute__((always_inline)) uint dm_find_smsb(int x) { return dm_find_umsb(uint(x < 0 ? ~x : x)); }
";

fn border() -> String {
    format!(
        "
struct dm_border_t {{ uint mask; float4 k[{}]; }};
static inline __attribute__((always_inline)) float4 dm_bmix(float4 r, float4 w, float4 k) {{ return r + (w - r) * k; }}
static inline __attribute__((always_inline)) float dm_bmix(float r, float w, float4 k) {{ return r + (w - r) * k.x; }}
template<typename V, typename T, typename... A> static inline __attribute__((always_inline)) V dm_bs(V r, constant dm_border_t* b, uint i, T t, sampler w, A... a)
{{
    if (b->mask >> i & 1u) r = dm_bmix(r, t.sample(w, a...), b->k[i]);
    return r;
}}
",
        abi::BORDER_SAMPLERS
    )
}

/// The helper functions a body asks for, in a fixed order.
pub fn helpers(used: &BTreeSet<&'static str>, border_struct: bool) -> String {
    let mut s = String::new();
    if border_struct {
        s.push_str(&border());
    }
    if used.contains("findmsb") {
        s.push_str(FIND_MSB);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_has_the_contract_pragma_unless_reduced() {
        assert!(header(false).contains("fp contract(off)"));
        assert!(!header(true).contains("fp contract"));
    }

    #[test]
    fn helpers_follow_the_request() {
        let mut u = BTreeSet::new();
        assert_eq!(helpers(&u, false), "");
        u.insert("findmsb");
        assert!(helpers(&u, true).contains("dm_bs"));
        assert!(helpers(&u, false).contains("dm_find_smsb"));
    }
}

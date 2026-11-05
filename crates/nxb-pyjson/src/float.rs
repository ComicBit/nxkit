//! Python's float `repr`: the shortest digits that round-trip, written `1e-05`, `1e+16`, `0.1`, `-0.0`, `inf`.

/// `repr(f)` as Python writes it (JSON writes `Infinity`, `-Infinity`, `NaN` for the non-finite ones, see `dumps`).
pub fn repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf".into() } else { "-inf".into() };
    }
    // `{:e}` gives the shortest round-trip digits: "d.ddddde<exp>".
    let e = format!("{:e}", f.abs());
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let sign = if f.is_sign_negative() { "-" } else { "" };
    let n = digits.len() as i32;
    if (-4..16).contains(&exp) {
        // Positional: the point after exp + 1 digits.
        let point = exp + 1;
        let s = if point <= 0 {
            format!("0.{}{}", "0".repeat((-point) as usize), digits)
        } else if point >= n {
            format!("{}{}.0", digits, "0".repeat((point - n) as usize))
        } else {
            let (a, b) = digits.split_at(point as usize);
            format!("{a}.{b}")
        };
        format!("{sign}{s}")
    } else {
        let m = if n == 1 {
            digits.clone()
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        let es = if exp < 0 { "-" } else { "+" };
        format!("{sign}{m}e{es}{:02}", exp.abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_examples() {
        for (f, s) in [
            (1.0, "1.0"),
            (2.5, "2.5"),
            (1e-05, "1e-05"),
            (1e16, "1e+16"),
            (123456789012345678.0, "1.2345678901234568e+17"),
            (0.1, "0.1"),
            (-0.0, "-0.0"),
            (0.0001, "0.0001"),
            (1234567890123456.0, "1234567890123456.0"),
            (1.5e-7, "1.5e-07"),
            (100.0, "100.0"),
            (0.30000000000000004, "0.30000000000000004"),
        ] {
            assert_eq!(repr(f), s, "{f}");
        }
    }
}

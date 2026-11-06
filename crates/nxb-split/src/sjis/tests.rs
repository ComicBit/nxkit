use super::*;

#[test]
fn ascii_and_katakana() {
    assert_eq!(decode(b"N:\\FRPG\\a~.vpo"), "N:\\FRPG\\a~.vpo");
    assert_eq!(decode(&[0xb1, 0xb2, 0xdf]), "\u{ff71}\u{ff72}\u{ff9f}");
}

#[test]
fn two_byte_characters_even_with_a_backslash_trail() {
    // 表 is 95 5c, ソ is 83 5c, ー is 81 5b
    assert_eq!(decode(&[0x95, 0x5c]), "表");
    assert_eq!(decode(&[0x83, 0x5c, 0x81, 0x5b, 0x83, 0x58]), "ソース");
    assert_eq!(decode(&[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea]), "日本語");
    assert_eq!(decode(&[0x81, 0x40]), "\u{3000}");
    assert_eq!(decode(&[0x81, 0x80]), "\u{f7}");
}

#[test]
fn bad_bytes_are_replaced_one_at_a_time() {
    assert_eq!(decode(&[0x80]), "\u{fffd}");
    assert_eq!(decode(&[0xa0, 0xfd, 0xff]), "\u{fffd}\u{fffd}\u{fffd}");
    assert_eq!(decode(&[0x81]), "\u{fffd}");
    assert_eq!(decode(&[0x81, 0x20]), "\u{fffd} ");
    assert_eq!(decode(&[0x81, 0x7f]), "\u{fffd}\u{7f}");
    assert_eq!(decode(&[0x81, 0xad]), "\u{fffd}\u{ff6d}");
    assert_eq!(decode(&[0x85, 0x40]), "\u{fffd}@");
    assert_eq!(decode(&[0xa1, 0x81]), "\u{ff61}\u{fffd}");
}

#[test]
fn the_table_has_the_6879_characters_of_jis_x_0208() {
    let n: usize = table::JIS0208
        .iter()
        .map(|r| r.iter().filter(|&&u| u != 0).count())
        .sum();
    assert_eq!(n, 6879);
}

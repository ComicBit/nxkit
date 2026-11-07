use super::Res;

fn res(kind: char, count: u32) -> Res {
    Res { kind, set: 1, binding: 3, count, slot: 4, globals: Vec::new() }
}

#[test]
fn token_has_kind_set_binding_and_slot() {
    assert_eq!(res('u', 1).token(), "u:1.3=4");
}

#[test]
fn token_of_an_array_has_its_count() {
    assert_eq!(res('t', 2).token(), "t:1.3=4*2");
}

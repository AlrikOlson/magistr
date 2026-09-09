use super::*;

#[test]
fn fields_and_sub_fields() {
    let p = Params::parse(b"1;5:3");
    assert_eq!(p.get(0), Some(1));
    assert_eq!(p.get(1), Some(5));
    assert_eq!(p.sub(1, 1), Some(3));
    assert_eq!(p.sub(1, 2), None);
    assert_eq!(p.get(2), None);
}

#[test]
fn empty_fields_are_none() {
    let p = Params::parse(b"97;;65");
    assert_eq!(p.get(0), Some(97));
    assert_eq!(p.get(1), None);
    assert_eq!(p.get(2), Some(65));
    let p = Params::parse(b"97:;2");
    assert_eq!(p.sub(0, 1), None);
    assert_eq!(p.get(1), Some(2));
    assert_eq!(Params::parse(b"").get(0), None);
}

#[test]
fn garbage_and_overflow() {
    assert_eq!(Params::parse(b"1x").get(0), None);
    assert_eq!(Params::parse(b"99999999999").get(0), Some(u32::MAX));
}

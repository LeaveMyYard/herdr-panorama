use super::*;

#[test]
fn a_panicking_orchestrator_becomes_an_error_and_the_rest_go_on() {
    assert_eq!(contain("w1:p1", || 7), Ok(7));
    let err = contain("w1:p2", || -> i32 {
        panic!("byte index 3 is not a char boundary")
    })
    .unwrap_err();
    assert_eq!(err, "w1:p2: panic: byte index 3 is not a char boundary");
    let long = contain("w1:p3", || -> i32 { panic!("{}", "x".repeat(400)) }).unwrap_err();
    assert_eq!(long.chars().count(), 300);
}

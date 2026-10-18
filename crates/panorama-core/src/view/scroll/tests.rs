use super::*;

const COLUMN: usize = 0;

#[test]
fn the_list_follows_the_selection_unless_the_wheel_moved_it() {
    let mut v = View::default();
    assert_eq!(offset(&v, COLUMN, Some((50, 54)), 41, 80), 13);
    assert_eq!(offset(&v, COLUMN, Some((0, 2)), 41, 80), 0);
    v.offsets.insert(COLUMN, 30);
    assert_eq!(offset(&v, COLUMN, Some((0, 2)), 41, 80), 0);
    v.wheel.insert(COLUMN);
    assert_eq!(offset(&v, COLUMN, Some((0, 2)), 41, 80), 30);
    assert_eq!(offset(&v, COLUMN, None, 41, 50), 9); // never past the content
}

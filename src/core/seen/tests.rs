use super::*;
use std::cell::Cell;
use std::hash::Hasher;

thread_local! {
    static EQS: Cell<u64> = const { Cell::new(0) };
}

/// An id that counts how often it is compared.
#[derive(Clone, Debug)]
struct Counted(u64);

impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        EQS.with(|n| n.set(n.get() + 1));
        self.0 == other.0
    }
}
impl Eq for Counted {}
impl Hash for Counted {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.0.hash(h);
    }
}

#[test]
fn seen_ids_are_kept_once_in_order() {
    let ids = ["a5", "a1", "a5", "a2", "a1"].map(String::from).to_vec();
    assert_eq!(
        once_each(ids, MAX_SEEN),
        (["a5", "a1", "a2"].map(String::from).to_vec(), false)
    );
    assert_eq!(once_each(Vec::<String>::new(), MAX_SEEN), (vec![], false));
}

#[test]
fn deduplication_is_linear() {
    // the review of #85: a quadratic dedup froze the core for seconds on one long line; this
    // counts comparisons instead of timing them (a quadratic one makes ~n²/2 here)
    let n = 20_000u64;
    let ids: Vec<Counted> = (0..n).chain(0..n).map(Counted).collect();
    EQS.with(|c| c.set(0));
    let (out, over) = once_each(ids, usize::MAX);
    assert_eq!(out.len() as u64, n);
    assert!(!over);
    let eqs = EQS.with(Cell::get);
    assert!(eqs <= 4 * n, "{eqs} comparisons for {n} ids");
}

#[test]
fn a_seen_list_is_capped() {
    let ids: Vec<u32> = (0..5000).collect();
    let (out, over) = once_each(ids, MAX_SEEN);
    assert_eq!(out, (0..MAX_SEEN as u32).collect::<Vec<_>>());
    assert!(over);
    let (out, over) = once_each((0..10).chain(0..10).collect(), 10);
    assert_eq!(
        (out.len(), over),
        (10, false),
        "repeats past the cap are no overflow"
    );
}

use super::*;

fn line(s: &str) -> Arc<Vec<u8>> {
    Arc::new(s.as_bytes().to_vec())
}

#[test]
fn a_full_outbox_loses_its_oldest() {
    let mut o = Outbox::new(4);
    for i in 0..6 {
        o.push(line(&i.to_string()));
    }
    let got: Vec<String> = std::iter::from_fn(|| o.pop())
        .map(|l| String::from_utf8(l.to_vec()).unwrap())
        .collect();
    assert_eq!(got, ["2", "3", "4", "5"]);
    assert!(o.is_empty());
}

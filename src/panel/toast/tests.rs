//! The toasts against the Textual notices captured at 160 columns
//! (`screens/demo-wide.json`: the help toast, then the transcript toast under it), cell for
//! cell over the toast area; and at a panel's width.

use super::*;
use serde_json::Value;

/// A golden cell: character, colour, background, bold.
type Cell = (char, Option<String>, Option<String>, bool);

fn golden(name: &str) -> Vec<Vec<Cell>> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/screens/demo-wide.json"
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    v["screens"][name]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            row.as_array()
                .unwrap()
                .iter()
                .flat_map(|r| {
                    let (fg, bg) = (
                        r[1].as_str().map(str::to_owned),
                        r[2].as_str().map(str::to_owned),
                    );
                    let bold = r[3].as_bool().unwrap_or(false);
                    r[0].as_str()
                        .unwrap()
                        .chars()
                        .map(move |c| (c, fg.clone(), bg.clone(), bold))
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .collect()
}

fn help() -> Toast {
    Toast::new(Some("keys"), crate::panel::input::actions::HELP, 10)
}

fn check(s: &Screen, want: &[Vec<Cell>], p: Placed) {
    for y in p.y0..p.y1 {
        for x in p.x0..p.x1 {
            let (ch, fg, bg, bold) = &want[y as usize][x as usize];
            let c = s.cell(x, y);
            assert_eq!(c.ch, *ch, "({x},{y})");
            assert_eq!(
                c.bg.map(|b| b.to_string()),
                bg.as_ref().map(|b| b.to_lowercase()),
                "bg ({x},{y})"
            );
            if *ch != ' ' {
                assert_eq!(
                    Some(c.fg.to_string()),
                    fg.as_ref().map(|f| f.to_lowercase()),
                    "fg ({x},{y})"
                );
                assert_eq!(c.bold, *bold, "bold ({x},{y})");
            }
        }
    }
}

#[test]
fn textuals_help_toast_cell_for_cell() {
    let t = Instant::now();
    let mut toasts = Toasts::default();
    toasts.push(help(), t);
    let mut s = Screen::new(160, 45);
    let placed = toasts.draw(&mut s);
    assert_eq!(
        placed,
        [Placed {
            x0: 98,
            x1: 158,
            y0: 36,
            y1: 44
        }]
    );
    check(&s, &golden("help toast"), placed[0]);
}

#[test]
fn a_second_toast_stacks_under_with_a_row_between() {
    let t = Instant::now();
    let mut toasts = Toasts::default();
    toasts.push(help(), t);
    let path = "/home/user/.claude/projects/-home-user-project/s/subagents/agent-b3a9c41.jsonl";
    toasts.push(Toast::new(Some("transcript"), path, 8), t);
    let mut s = Screen::new(160, 45);
    let placed = toasts.draw(&mut s);
    assert_eq!(
        placed[1],
        Placed {
            x0: 98,
            x1: 158,
            y0: 39,
            y1: 44
        }
    );
    let want = golden("transcript toast");
    check(&s, &want, placed[1]);
    check(&s, &want, placed[0]);
}

#[test]
fn at_a_panels_width_a_toast_takes_half_the_rack_and_expires() {
    let t = Instant::now();
    let mut toasts = Toasts::default();
    toasts.push(Toast::new(None, "panorama is restarting…", 3).warning(), t);
    let mut s = Screen::new(46, 45);
    let placed = toasts.draw(&mut s);
    assert_eq!(
        placed,
        [Placed {
            x0: 22,
            x1: 44,
            y0: 40,
            y1: 44
        }]
    );
    assert_eq!(s.cell(22, 41).fg, WARNING.0);
    assert_eq!(toasts.next_expiry(), Some(t + Duration::from_secs(3)));
    assert!(!toasts.expire(t + Duration::from_secs(2)));
    assert!(toasts.expire(t + Duration::from_secs(3)));
    assert!(toasts.next_expiry().is_none());
}

#[test]
fn a_click_on_a_toast_dismisses_it() {
    let t = Instant::now();
    let mut toasts = Toasts::default();
    toasts.push(help(), t);
    let placed = toasts.draw(&mut Screen::new(160, 45));
    assert!(!toasts.dismiss_at(5, 5, &placed));
    assert!(toasts.dismiss_at(100, 40, &placed));
    assert!(toasts.next_expiry().is_none());
}

#[test]
fn rich_folds_long_words() {
    assert_eq!(fold("ab cd ef", 5), ["ab cd", "ef"]);
    assert_eq!(fold("abcdefghij k", 4), ["abcd", "efgh", "ij k"]);
}

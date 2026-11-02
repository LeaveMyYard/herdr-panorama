use super::Screen;
use serde_json::{Value, json};
use std::cell::Cell;
use std::io::Write;
use std::sync::Mutex;
thread_local! { static LEGACY: Cell<bool> = const { Cell::new(false) }; }
static LOCK: Mutex<()> = Mutex::new(());
pub fn legacy() -> bool { LEGACY.with(Cell::get) }
fn row(s: &Screen, y:i32)->Value {
    let mut runs: Vec<Value> = vec![];
    for x in 0..s.w() { let c=s.cell(x,y); let fg=if c.ch==' ' {None} else {Some(c.fg.to_string())}; let bg=c.bg.map_or("#101215".into(),|v|v.to_string()); let bold=c.ch!=' ' && c.bold;
       if let Some(last)=runs.last_mut() { if last[1]==json!(fg) && last[2]==json!(bg) && last[3]==json!(bold) { let text=last[0].as_str().unwrap().to_owned()+&c.ch.to_string();last[0]=json!(text);continue; } }
       runs.push(json!([c.ch.to_string(),fg,bg,bold]));
    }
    json!(runs)
}
pub fn record(new:&Screen, old:impl FnOnce()->Screen) {
    let Ok(path)=std::env::var("PANORAMA_CAPTURE_ROWS") else {return};
    if legacy(){return}
    LEGACY.with(|v|v.set(true));let old=old();LEGACY.with(|v|v.set(false));
    let _lock=LOCK.lock().unwrap();
    let mut file=std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
    for y in 0..old.h().min(new.h()) { let a=row(&old,y);let b=row(new,y);if a!=b { writeln!(file,"{}",json!({"before":a,"after":b})).unwrap(); } }
}

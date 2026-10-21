//! `panorama emit`'s arguments, as Python's argparse parser for it reads them: `--opt value` or
//! `--opt=value`, a unique prefix of an option for the option (`--thr`), `-h`/`--help` for the
//! help (exit 0), `--phase` and `--agent` repeating and the others taking their last value, a
//! value that looks like an option refused, and argparse's errors with its usage line (exit 2).
//! The help and the usage lines are argparse's own (`help.txt`, `top-usage.txt`).

/// The parsed arguments.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Args {
    pub session: Option<String>,
    pub thread: String,
    pub name: Option<String>,
    pub flavour: Option<String>,
    pub status: Option<String>,
    pub worktree: Option<String>,
    pub pct: Option<i64>,
    pub phases: Vec<String>,
    pub agents: Vec<String>,
    pub json: Option<String>,
}

/// What the arguments ask for.
#[derive(Clone, Debug, PartialEq)]
pub enum Parsed {
    Help,
    Emit(Box<Args>),
}

/// argparse's `emit -h`, word for word.
pub const HELP: &str = include_str!("help.txt");
/// The `panorama` parser's usage, which argparse prints for unrecognized arguments.
pub const TOP_USAGE: &str = include_str!("top-usage.txt");

/// The statuses `--status` takes.
pub const STATUSES: [&str; 4] = ["active", "done", "failed", "cancelled"];

const OPTIONS: [&str; 11] = [
    "--help",
    "--session",
    "--thread",
    "--name",
    "--flavour",
    "--status",
    "--worktree",
    "--pct",
    "--phase",
    "--agent",
    "--json",
];

/// The emit parser's usage (the help's first lines).
pub fn usage() -> String {
    HELP.split("\n\n").next().unwrap_or("").to_owned() + "\n"
}

/// An error as argparse prints it (usage, then `prog: error: …`), exit 2.
fn error(msg: String) -> String {
    format!("{}panorama emit: error: {msg}", usage())
}

/// `int()` of a string: trimmed, a sign, digits with single underscores between them.
fn py_int(s: &str) -> Option<i64> {
    let t = s.trim();
    let digits = t.strip_prefix(['+', '-']).unwrap_or(t);
    let ok = !digits.is_empty()
        && !digits.starts_with('_')
        && !digits.ends_with('_')
        && !digits.contains("__")
        && digits.chars().all(|c| c.is_ascii_digit() || c == '_');
    ok.then(|| t.replace('_', "").parse().ok()).flatten()
}

/// Whether argparse takes `s` for an option (not a value): a leading `-`, unless it reads as
/// a negative number.
fn looks_like_option(s: &str) -> bool {
    s.starts_with('-') && s.len() > 1 && s[1..].parse::<f64>().is_err()
}

/// The long option a flag names: itself, or the one option it is a unique prefix of.
fn resolve(flag: &str) -> Result<Option<&'static str>, String> {
    if let Some(exact) = OPTIONS.iter().find(|o| **o == flag) {
        return Ok(Some(exact));
    }
    let hits: Vec<&str> = OPTIONS
        .iter()
        .copied()
        .filter(|o| o.starts_with(flag))
        .collect();
    match hits.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one)),
        many => Err(error(format!(
            "ambiguous option: {flag} could match {}",
            many.join(", ")
        ))),
    }
}

/// The arguments after `emit`: the help, the args, or argparse's error text.
pub fn parse(argv: &[String]) -> Result<Parsed, String> {
    let mut a = Args::default();
    let mut thread = None;
    let mut extras: Vec<String> = Vec::new();
    let mut it = argv.iter();
    while let Some(arg) = it.next() {
        if arg == "-h" {
            return Ok(Parsed::Help);
        }
        if !arg.starts_with("--") || arg == "--" {
            extras.push(arg.clone());
            continue;
        }
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) => (f, Some(v.to_owned())),
            None => (arg.as_str(), None),
        };
        let Some(opt) = resolve(flag)? else {
            extras.push(arg.clone());
            continue;
        };
        if opt == "--help" {
            return Ok(Parsed::Help);
        }
        let value = match inline {
            Some(v) => v,
            None => match it.as_slice().first().filter(|v| !looks_like_option(v)) {
                Some(v) => {
                    it.next();
                    v.clone()
                }
                None => return Err(error(format!("argument {opt}: expected one argument"))),
            },
        };
        match opt {
            "--session" => a.session = Some(value),
            "--thread" => thread = Some(value),
            "--name" => a.name = Some(value),
            "--flavour" => a.flavour = Some(value),
            "--status" if STATUSES.contains(&value.as_str()) => a.status = Some(value),
            "--status" => {
                let msg = format!(
                    "argument --status: invalid choice: '{value}' (choose from {})",
                    STATUSES.join(", ")
                );
                return Err(error(msg));
            }
            "--worktree" => a.worktree = Some(value),
            "--pct" => {
                a.pct = Some(py_int(&value).ok_or_else(|| {
                    error(format!("argument --pct: invalid int value: '{value}'"))
                })?)
            }
            "--phase" => a.phases.push(value),
            "--agent" => a.agents.push(value),
            _ => a.json = Some(value),
        }
    }
    a.thread =
        thread.ok_or_else(|| error("the following arguments are required: --thread".into()))?;
    if !extras.is_empty() {
        return Err(format!(
            "{TOP_USAGE}panorama: error: unrecognized arguments: {}",
            extras.join(" ")
        ));
    }
    Ok(Parsed::Emit(Box::new(a)))
}

#[cfg(test)]
mod tests;

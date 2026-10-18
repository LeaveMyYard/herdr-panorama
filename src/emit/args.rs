//! `panorama emit`'s arguments (Python `cli.main`'s `emit` parser): `--opt value` or
//! `--opt=value`; `--phase` and `--agent` repeat, the others take their last value.

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

/// The statuses `--status` takes.
pub const STATUSES: [&str; 4] = ["active", "done", "failed", "cancelled"];

/// The arguments after `emit`, or why not (argparse's exit 2).
pub fn parse(argv: &[String]) -> Result<Args, String> {
    let mut a = Args::default();
    let mut thread = None;
    let mut it = argv.iter();
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_owned())),
            _ => (arg.as_str(), None),
        };
        let known = [
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
        if !known.contains(&flag) {
            return Err(format!("unrecognized arguments: {arg}"));
        }
        let value = match inline {
            Some(v) => v,
            None => it
                .next()
                .cloned()
                .ok_or_else(|| format!("argument {flag}: expected one argument"))?,
        };
        match flag {
            "--session" => a.session = Some(value),
            "--thread" => thread = Some(value),
            "--name" => a.name = Some(value),
            "--flavour" => a.flavour = Some(value),
            "--status" if STATUSES.contains(&value.as_str()) => a.status = Some(value),
            "--status" => return Err(format!("argument --status: invalid choice: '{value}'")),
            "--worktree" => a.worktree = Some(value),
            "--pct" => {
                a.pct = Some(
                    value
                        .trim()
                        .parse()
                        .map_err(|_| format!("argument --pct: invalid int value: '{value}'"))?,
                )
            }
            "--phase" => a.phases.push(value),
            "--agent" => a.agents.push(value),
            _ => a.json = Some(value),
        }
    }
    a.thread = thread.ok_or("the following arguments are required: --thread")?;
    Ok(a)
}

#[cfg(test)]
mod tests;

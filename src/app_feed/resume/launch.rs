//! Which launches a resume command can honestly repeat.
use super::state::Launch;
use herdr_agent_kit::agent_session::SessionId;

/// How a process was started, by allow-list: anything not named here is `Other`.
pub(super) fn launch(harness: &str, argv: &[String]) -> (Launch, bool, bool) {
    let (mut dropped, mut no_daemon) = (false, false);
    let mut args = argv.iter().skip(1).map(String::as_str);
    let uuid = |value: Option<&str>| value.is_some_and(|v| SessionId::parse(v).is_some());
    while let Some(arg) = args.next() {
        let known = match (harness, arg) {
            ("claude", "--resume" | "-r" | "--session-id") => uuid(args.next()),
            ("claude", "--continue" | "-c") => true,
            ("claude", "--model" | "--effort") | ("codex", "--model" | "-m") => {
                dropped = true;
                args.next().is_some()
            }
            ("claude", "--verbose") => {
                dropped = true;
                true
            }
            ("codex", "--no-daemon") => {
                no_daemon = true;
                true
            }
            ("codex", "resume") => args.next().is_none_or(|v| uuid(Some(v))),
            _ => false,
        };
        if !known {
            return (Launch::Other, dropped, no_daemon);
        }
    }
    (Launch::Plain, dropped, no_daemon)
}

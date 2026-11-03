use super::{ensure, fs::Home, json};
use std::path::PathBuf;

pub const HELP: &str =
    "panorama app-feed v1\nusage: panorama app-feed --ensure [--app-home ABSOLUTE_HOME]\n";
fn arguments(args: &[String]) -> Result<(bool, PathBuf, Option<String>), String> {
    let mut ensure = false;
    let mut watch = false;
    let mut home = None;
    let mut id = None;
    let mut at = 0;
    while at < args.len() {
        match args[at].as_str() {
            "--ensure" if !ensure => ensure = true,
            "--watch" if !watch => watch = true,
            "--app-home" if home.is_none() && at + 1 < args.len() => {
                at += 1;
                home = Some(PathBuf::from(&args[at]));
            }
            "--install-id" if id.is_none() && at + 1 < args.len() => {
                at += 1;
                id = Some(args[at].clone());
            }
            _ => return Err(HELP.into()),
        }
        at += 1;
    }
    if ensure == watch
        || (ensure && id.is_some())
        || (watch && (home.is_none() || id.as_deref().is_none_or(|s| !json::hex(s, 32))))
    {
        return Err(HELP.into());
    }
    let home = home
        .or_else(|| {
            std::env::var_os("HERDR_APP_HOME")
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| {
            herdr_agent_kit::plugin_env::PluginEnv::from_env()
                .home()
                .map(|p| p.join(".config/herdr-app"))
        })
        .ok_or("app home is unavailable; pass --app-home")?;
    Ok((ensure, home, id))
}
pub fn run(args: &[String]) -> i32 {
    if args == ["--help"] {
        print!("{HELP}");
        return 0;
    }
    let result = arguments(args).and_then(|(start, path, id)| {
        let home = Home::open(&path)?;
        if start {
            ensure::ensure(home)
        } else {
            ensure::serve(home, id.as_deref().unwrap())
        }
    });
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("panorama app-feed: {error}");
            1
        }
    }
}

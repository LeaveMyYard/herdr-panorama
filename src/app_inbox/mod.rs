//! Optional navigation through the existing installed Inbox CLI, behind private artifact proof.
pub mod runner;
use crate::{
    app_feed::{
        fs::{self, Home},
        receipt::Receipt,
    },
    panel::act::inbox::InboxLink,
};
use herdr_agent_kit::herdr_api::Herdr;
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub const HELP: &str = "panorama app-inbox v1\nusage: panorama app-inbox (--available | --open [--question POSITIVE_SAFE_ID]) --app-home ABSOLUTE_HOME\n";
#[derive(Debug, PartialEq)]
pub struct Args {
    pub home: PathBuf,
    pub open: bool,
    pub question: Option<i64>,
}
pub fn arguments(args: &[String]) -> Result<Args, String> {
    let (mut available, mut open, mut home, mut question) = (false, false, None, None);
    let mut at = 0;
    while at < args.len() {
        match args[at].as_str() {
            "--available" if !available => available = true,
            "--open" if !open => open = true,
            "--app-home" if home.is_none() && at + 1 < args.len() => {
                at += 1;
                home = Some(PathBuf::from(&args[at]));
            }
            "--question" if question.is_none() && at + 1 < args.len() => {
                at += 1;
                let s = &args[at];
                if s.starts_with('0') || !s.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(HELP.into());
                }
                question = Some(
                    s.parse::<i64>()
                        .ok()
                        .filter(|q| (1..=9_007_199_254_740_991).contains(q))
                        .ok_or(HELP)?,
                );
            }
            _ => return Err(HELP.into()),
        }
        at += 1;
    }
    let home = home.filter(|p| p.is_absolute()).ok_or(HELP)?;
    if available == open || available && question.is_some() {
        return Err(HELP.into());
    }
    Ok(Args {
        home,
        open,
        question,
    })
}
fn proof(args: &Args, deadline: Instant) -> Result<(), String> {
    let home = Home::open(&args.home)?;
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or("navigation timed out")?;
    fs::lock(&home.fd, remaining)?;
    let receipt = Receipt::read(&home)?;
    receipt.check_artifact_until(&home, Some(deadline))?;
    receipt.enabled(&home)?;
    if Instant::now() >= deadline {
        return Err("navigation timed out".into());
    }
    Ok(())
}
async fn navigate(args: &Args, h: &Herdr, deadline: Instant) -> Result<Value, String> {
    let run = runner::InstalledRunner { deadline };
    let installed = InboxLink::default().installed(h, &run).await;
    if !args.open {
        return Ok(installed.map_or(
            json!({"available":false,"questions":false}),
            |i| json!({"available":true,"questions":i.questions}),
        ));
    }
    let installed = installed.ok_or("installed inbox unavailable")?;
    // Capability discovery may wait; uninstall or artifact replacement invalidates it.
    proof(args, deadline)?;
    installed.open(&run, args.question).await?;
    Ok(json!({"opened":true}))
}
pub fn run(args: &[String]) -> i32 {
    if args == ["--help"] {
        print!("{HELP}");
        return 0;
    }
    let args = match arguments(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return 2;
        }
    };
    let deadline = Instant::now() + Duration::from_millis(4800);
    let result = proof(&args, deadline).and_then(|()| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let result = rt.block_on(async {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("navigation timed out")?;
            let h = Herdr::from_env().with_client("panorama-app-inbox");
            tokio::time::timeout(remaining, navigate(&args, &h, deadline))
                .await
                .map_err(|_| "navigation timed out; no retry".to_owned())?
        });
        // The bounded helper's own deadline expires concurrently; reap its owned handle.
        rt.shutdown_timeout(Duration::from_millis(100));
        result
    });
    match result {
        Ok(v) => {
            println!("{v}");
            0
        }
        Err(e) => {
            eprintln!("panorama app-inbox: {e}");
            1
        }
    }
}
#[cfg(test)]
mod tests;

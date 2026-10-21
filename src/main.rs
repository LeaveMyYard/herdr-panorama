//! The `panorama` binary (plan §2.2): one binary, subcommands keep today's CLI names. Ported
//! so far: `panel`, the per-tab panel viewer; `tui`, the graph view; `new-agent`, the + New
//! agent popup; `open` and `home`, the plugin's actions and startup hook; `snapshot`, the
//! derived model; `contract`, the contracts the launchers check (`scripts/panel`,
//! `scripts/tui`, the Claude Code side's); `hook` and `mcp`, the Claude Code side. Without a
//! subcommand it prints its version; an unknown one exits 2.
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("panel") => std::process::exit(panorama::panel::run()),
        Some("tui") => std::process::exit(panorama::app::run::tui()),
        Some("contract") => match panorama::contract::answer(std::env::args().nth(2).as_deref()) {
            Some(answer) => println!("{answer}"),
            None => std::process::exit(2),
        },
        Some("hook") => std::process::exit(panorama::hook::main(
            std::env::args().nth(2).as_deref().unwrap_or(""),
        )),
        Some("mcp") => std::process::exit(panorama::mcp::run()),
        Some("emit") => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            std::process::exit(panorama::emit::run(&args))
        }
        Some(cmd @ ("open" | "home")) => {
            let rest: Vec<String> = std::env::args().skip(2).collect();
            std::process::exit(panorama::open::main(cmd, &rest))
        }
        Some("new-agent") => std::process::exit(panorama::new_agent::popup::run()),
        Some("snapshot") => {
            let args: Vec<String> = std::env::args().skip(2).collect();
            std::process::exit(panorama::snapshot::run(&args))
        }
        None => println!("panorama {}", env!("CARGO_PKG_VERSION")),
        // a subcommand this binary does not know: nothing on stdout (a launcher reading it must
        // never take the version line for an answer), and a failure
        Some(other) => {
            eprintln!("panorama: unknown subcommand {other:?}");
            std::process::exit(2)
        }
    }
}

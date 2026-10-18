//! The `panorama` binary (plan §2.2): one binary, subcommands keep today's CLI names. Ported
//! so far: `panel`, the per-tab panel viewer, and `contract`, the panel contract it implements
//! (`scripts/panel` checks it before running `panel`). Without a subcommand it prints its version.

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("panel") => std::process::exit(panorama::panel::run()),
        Some("contract") => println!("{}", panorama::contract::PANEL),
        _ => println!("panorama {}", env!("CARGO_PKG_VERSION")),
    }
}

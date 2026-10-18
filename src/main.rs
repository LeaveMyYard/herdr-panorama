//! The `panorama` binary (plan §2.2): one binary, subcommands keep today's CLI names. Ported
//! so far: `panel`, the per-tab panel viewer. Without a subcommand it prints its version.

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("panel") => std::process::exit(panorama::panel::run()),
        _ => println!("panorama {}", env!("CARGO_PKG_VERSION")),
    }
}

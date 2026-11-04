//! `panorama emit`: deprecated. It declared a thread on the explicit channel by hand; threads
//! were removed (plan `uniform-agent-tree.md` §6), so it takes any arguments (the old flags
//! included, and `--help`), records nothing, says so on stderr and exits 0, so scripts that call
//! it keep working. Release N+1 removes it.

/// What `emit` says, on stderr.
pub const NOTE: &str = "panorama emit: threads were removed; nothing recorded";

/// `panorama emit <argv>`; the exit status (0).
pub fn run(_argv: &[String]) -> i32 {
    eprintln!("{NOTE}");
    0
}

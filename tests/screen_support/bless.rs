/// Whether this run rewrites the Rust-owned goldens (`tests/fixtures/rust/`) instead of
/// comparing against them: `PANORAMA_BLESS=1`. The rewrite is then reviewed as a diff.
pub fn blessing() -> bool {
    std::env::var_os("PANORAMA_BLESS").is_some_and(|v| v == "1")
}

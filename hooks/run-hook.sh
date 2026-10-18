# herdr-panorama: run a stdlib Python hook script with a cached interpreter.
#
#   sh run-hook.sh <script.py> <args...>
#
# `python3` is often a version-manager shim (pyenv's is a bash script costing 60-75 ms per
# call). The interpreter it resolves to is cached in Claude Code's per-plugin data directory
# ($CLAUDE_PLUGIN_DATA/python-interpreter) and executed directly afterwards.
#
# Only an interpreter new enough for the hook scripts (3.10+) is cached: python3 is resolved in
# the first hook's cwd, so a project's pyenv `.python-version` or an active venv decides what
# gets cached, and an older one must not become every session's interpreter.
#
# Safe by construction: a cache that is unreadable, not an absolute path or no longer an
# executable file is ignored and re-resolved; the cache is written to a per-process temp
# file and renamed into place (concurrent hooks only ever see a whole file, and all write
# the same answer); an unwritable directory or a missing CLAUDE_PLUGIN_DATA just means plain
# `python3`. hooks.json discards this script's stderr and forces exit 0 around it.

hook=$1
shift

# The Rust hooks first (docs/plans/rust-rewrite.md §5.4): when this plugin root has the binary
# built (target/release/panorama: a linked checkout's `cargo build --release`, or a copy of one)
# and it speaks this checkout's Claude-side contract (`panorama contract claude` answers
# `claude <n>`: a binary from before the contract answers a bare number and is passed over),
# it runs the hook (`panorama hook <kind>`, same records, same output, exit 0). Anything else
# (no binary, a stale one, one that fails on the probe, or hangs on it: 2 s with timeout(1),
# then KILL; where there is no timeout(1), as on stock macOS, perl's alarm ends the probed
# process after 2 s (only that process: a child it left holding the output would keep the
# probe waiting); unbounded with neither) falls through to Python.
CLAUDE_CONTRACT=1
case $0 in
*/*) here=${0%/*} ;;
*) here=. ;;
esac
root=$(cd "$here/.." 2>/dev/null && pwd)
bin=$root/target/release/panorama
if [ -n "$root" ] && [ -f "$bin" ] && [ -x "$bin" ]; then
	if command -v timeout >/dev/null 2>&1; then
		got=$(timeout -k 1 2 "$bin" contract claude </dev/null 2>/dev/null)
	elif command -v perl >/dev/null 2>&1; then
		got=$(perl -e 'alarm shift; exec @ARGV or exit 127' 2 "$bin" contract claude </dev/null 2>/dev/null)
	else
		got=$("$bin" contract claude </dev/null 2>/dev/null)
	fi
	if [ "$got" = "claude $CLAUDE_CONTRACT" ]; then
		exec "$bin" hook "$@"
	fi
fi

usable() {
	case $1 in
	/*) [ -f "$1" ] && [ -x "$1" ] ;;
	*) return 1 ;;
	esac
}

if [ -n "${CLAUDE_PLUGIN_DATA:-}" ]; then
	cache=$CLAUDE_PLUGIN_DATA/python-interpreter
	py=
	[ -r "$cache" ] && IFS= read -r py <"$cache"
	if usable "$py"; then
		exec "$py" -I "$hook" "$@"
	fi
	py=$(python3 -I -c 'import sys; print(sys.executable or "" if sys.version_info >= (3, 10) else "")' \
		2>/dev/null </dev/null)
	if usable "$py"; then
		tmp=$cache.$$.tmp
		if [ -d "$cache" ]; then
			: # a directory in the cache's place: never write into it
		elif mkdir -p "$CLAUDE_PLUGIN_DATA" 2>/dev/null && printf '%s\n' "$py" >"$tmp" 2>/dev/null; then
			mv -f "$tmp" "$cache" 2>/dev/null || rm -f "$tmp" 2>/dev/null
		else
			rm -f "$tmp" 2>/dev/null
		fi
		exec "$py" -I "$hook" "$@"
	fi
fi
exec python3 -I "$hook" "$@"

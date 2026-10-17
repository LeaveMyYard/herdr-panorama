# herdr-panorama: run a stdlib Python hook script with a cached interpreter.
#
#   sh run-hook.sh <script.py> <args...>
#
# `python3` is often a version-manager shim (pyenv's is a bash script costing 60-75 ms per
# call). The interpreter it resolves to is cached in Claude Code's per-plugin data directory
# ($CLAUDE_PLUGIN_DATA/python-interpreter) and executed directly afterwards.
#
# Safe by construction: a cache that is unreadable, not an absolute path or no longer an
# executable file is ignored and re-resolved; the cache is written to a per-process temp
# file and renamed into place (concurrent hooks only ever see a whole file, and all write
# the same answer); an unwritable directory or a missing CLAUDE_PLUGIN_DATA just means plain
# `python3`. hooks.json discards this script's stderr and forces exit 0 around it.

hook=$1
shift

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
	py=$(python3 -I -c 'import sys; print(sys.executable or "")' 2>/dev/null </dev/null)
	if usable "$py"; then
		tmp=$cache.$$.tmp
		if mkdir -p "$CLAUDE_PLUGIN_DATA" 2>/dev/null && printf '%s\n' "$py" >"$tmp" 2>/dev/null; then
			mv -f "$tmp" "$cache" 2>/dev/null || rm -f "$tmp" 2>/dev/null
		else
			rm -f "$tmp" 2>/dev/null
		fi
		exec "$py" -I "$hook" "$@"
	fi
fi
exec python3 -I "$hook" "$@"

# bash completion for `git shook`.
#
# git's own completion dispatches to `_git_<name>` for a subcommand it does not
# know, with the underscore replacing the dash, so the function name is fixed
# and is not ours to choose.
#
#   source completions/git-shook.bash
#
# Needs git's `git-completion.bash` already sourced, which is what defines the
# `_git_*` dispatch this hooks into.

_git_shook() {
	local cur prev
	cur="${COMP_WORDS[COMP_CWORD]}"
	prev="${COMP_WORDS[COMP_CWORD - 1]}"

	case "$prev" in
	install)
		COMPREPLY=($(compgen -W "--yes" -- "$cur"))
		return
		;;
	esac

	if [ "$COMP_CWORD" -le 2 ]; then
		COMPREPLY=($(compgen -W "install list doctor uninstall version help" -- "$cur"))
	fi
}

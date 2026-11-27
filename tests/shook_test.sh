#!/usr/bin/env nutshell
# =============================================================================
# shook_test.sh - the whole behaviour of the base, as a suite any implementation
# of it is held to
# =============================================================================
#
# Every line of the list at the top of `shook` is a test here, and nothing the
# list does not say is. A second implementation of the base runs the same file:
#
#   ./tests/shook_test.sh                          the POSIX base beside this file
#   SHOOK_BASE=/path/to/other ./tests/shook_test.sh  any other
#
# Mutation-checked against a copy of `shook`, each mutation failing the test
# named beside it:
#
#   sort the entries in reverse                entries_run_in_the_byte_order_of_their_names
#   run every entry in the root                an_entry_runs_in_its_own_directory_under_the_root
#   append nothing after the entry's own       the_arguments_after_the_chain_reach_every_entry
#   run an entry with no argument              an_entry_with_no_argument_is_skipped
#   hand `replay` entries /dev/null            replay_hands_every_entry_the_whole_stdin
#   read stdin under `none`                    none_hands_every_entry_nothing
#   replay under `pass`                        pass_hands_the_entries_the_stream_itself
#   carry on past a failure                    the_first_failure_stops_the_chain_and_is_its_status
#   exit 1 where a directory is missing        a_directory_that_cannot_be_entered_is_an_exit_of_127
#   exit 1 on a mistake in the call            a_mistake_in_the_call_exits_64
#   run the entry through `sh -c`              an_argument_is_one_argument_whatever_it_holds

use test log

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE="${SHOOK_BASE:-$HERE/shook}"

# A chain at <dir>/<chain>, one entry per call to `an_entry`.
an_entry() { # <chain dir> <name> <working dir or empty> <argv...>
    local c="$1" n="$2" wd="$3"; shift 3
    mkdir -p "$c"
    {
        printf '%s\n' "$wd"
        local a
        for a in "$@"; do printf '%s\n' "$a"; done
    } > "$c/$n"
}

#[test]
an_absent_chain_passes() {
    local w; w="$(mktemp -d)"
    assert_exits 0 "$BASE" run --root "$w" "$w/nothing-here"
    mkdir "$w/empty"
    assert_exits 0 "$BASE" run --root "$w" "$w/empty"
    rm -rf "$w"
}

#[test]
entries_run_in_the_byte_order_of_their_names() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" b "" sh -c 'printf b'
    an_entry "$w/c" a "" sh -c 'printf a'
    an_entry "$w/c" B "" sh -c 'printf B'
    an_entry "$w/c" _ "" sh -c 'printf _'
    assert_eq "$("$BASE" run --root "$w" "$w/c")" "B_ab"
    rm -rf "$w"
}

#[test]
an_entry_runs_in_its_own_directory_under_the_root() {
    local w; w="$(mktemp -d)"
    mkdir -p "$w/sub/deeper"
    an_entry "$w/c" 1 "" pwd
    an_entry "$w/c" 2 "sub/deeper" pwd
    local out; out="$(cd / && "$BASE" run --root "$w" "$w/c")"
    assert_eq "$out" "$(cd "$w" && pwd)
$(cd "$w/sub/deeper" && pwd)"
    rm -rf "$w"
}

#[test]
the_root_is_the_current_directory_where_none_is_given() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" 1 "" pwd
    assert_eq "$(cd "$w" && "$BASE" run c)" "$(cd "$w" && pwd)"
    rm -rf "$w"
}

#[test]
the_arguments_after_the_chain_reach_every_entry() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" 1 "" printf '%s|' one
    an_entry "$w/c" 2 "" printf '%s|' two
    assert_eq "$("$BASE" run --root "$w" "$w/c" x y)" "one|x|y|two|x|y|"
    rm -rf "$w"
}

#[test]
an_entry_with_no_argument_is_skipped() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" 1 ""
    an_entry "$w/c" 2 "" printf "%s" ran
    # the arguments after the chain are not a program of their own
    assert_eq "$("$BASE" run --root "$w" "$w/c" false)" "ranfalse"
    rm -rf "$w"
}

#[test]
none_hands_every_entry_nothing() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" 1 "" cat
    assert_eq "$(printf 'fed\n' | "$BASE" run --root "$w" "$w/c")" ""
    assert_eq "$(printf 'fed\n' | "$BASE" run --root "$w" --stdin none "$w/c")" ""
    rm -rf "$w"
}

#[test]
replay_hands_every_entry_the_whole_stdin() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" 1 "" cat
    an_entry "$w/c" 2 "" cat
    assert_eq "$(printf 'one\ntwo\n' | "$BASE" run --root "$w" --stdin replay "$w/c")" "one
two
one
two"
    rm -rf "$w"
}

#[test]
pass_hands_the_entries_the_stream_itself() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" 1 "" sh -c 'read -r l; printf "1:%s\n" "$l"'
    an_entry "$w/c" 2 "" sh -c 'read -r l; printf "2:%s\n" "$l"'
    assert_eq "$(printf 'one\ntwo\n' | "$BASE" run --root "$w" --stdin pass "$w/c")" "1:one
2:two"
    rm -rf "$w"
}

#[test]
stdout_and_stderr_are_the_entries_own() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" 1 "" sh -c 'printf out; printf err >&2'
    assert_eq "$("$BASE" run --root "$w" "$w/c" 2>/dev/null)" "out"
    assert_eq "$("$BASE" run --root "$w" "$w/c" 2>&1 >/dev/null)" "err"
    rm -rf "$w"
}

#[test]
the_first_failure_stops_the_chain_and_is_its_status() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" 1 "" printf before
    an_entry "$w/c" 2 "" sh -c 'exit 3'
    an_entry "$w/c" 3 "" printf SHOULD-NOT-RUN
    local out rc=0
    out="$("$BASE" run --root "$w" "$w/c")" || rc=$?
    assert_eq "$rc" 3
    assert_eq "$out" before
    rm -rf "$w"
}

#[test]
a_chain_that_ran_through_exits_0() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" 1 "" true
    an_entry "$w/c" 2 "" true
    assert_exits 0 "$BASE" run --root "$w" "$w/c"
    rm -rf "$w"
}

#[test]
a_directory_that_cannot_be_entered_is_an_exit_of_127() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" 1 "not-there" printf SHOULD-NOT-RUN
    an_entry "$w/c" 2 "" printf SHOULD-NOT-RUN
    local out rc=0
    out="$("$BASE" run --root "$w" "$w/c")" || rc=$?
    assert_eq "$rc" 127
    assert_eq "$out" ""
    rm -rf "$w"
}

#[test]
a_mistake_in_the_call_exits_64() {
    local w; w="$(mktemp -d)"
    : > "$w/a-file"
    assert_exits 64 "$BASE" run
    assert_exits 64 "$BASE" run --stdin sideways "$w"
    assert_exits 64 "$BASE" run --stdin
    assert_exits 64 "$BASE" run --root
    assert_exits 64 "$BASE" run --nonsense "$w"
    assert_exits 64 "$BASE" run "$w/a-file"
    assert_exits 64 "$BASE" sideways
    rm -rf "$w"
}

#[test]
an_argument_is_one_argument_whatever_it_holds() {
    local w; w="$(mktemp -d)"
    an_entry "$w/c" 1 "" printf '<%s>' 'two words' '*' '$HOME' '; echo no'
    assert_eq "$("$BASE" run --root "$w" "$w/c" 'a b')" '<two words><*><$HOME><; echo no><a b>'
    rm -rf "$w"
}

# `test_run` sources this file once per test, so the run block guards against
# starting a second suite inside the first.
if [[ -z "${_SHOOK_BASE_TEST_RUNNING:-}" ]]; then
    export _SHOOK_BASE_TEST_RUNNING=1
    test_run "${BASH_SOURCE[0]}"
    test_summary
    exit $?
fi

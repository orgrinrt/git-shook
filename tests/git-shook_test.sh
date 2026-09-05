#!/usr/bin/env nutshell
# =============================================================================
# git-shook_test.sh - what the dispatcher runs, in what order, and what it refuses
# =============================================================================
# Every check here was first run by hand while the tool was written, against a
# scratch repository with two manifests in it. It is kept so nobody runs it by
# hand again.
#
# Each test builds its own repository in a temporary directory and points at the
# real `git-shook` beside this file, so nothing here touches the repository it is
# run from.
#
#   ./tests/git-shook_test.sh
#   TEST_FILTER=order ./tests/git-shook_test.sh
#
# Mutation-checked rather than trusted for being green. Five mutations were run
# against a copy of the tree, and each failed exactly the test named beside it
# and nothing else, 16 passing in every case:
#
#   drop the stdin replay from the        every_entry_sees_the_stdin_git_gave
#     dispatcher
#   run entries without `cd "$root/$wd"`  an_entry_runs_in_its_manifests_directory
#   drop the `break` so a failure does    a_failing_entry_stops_the_ones_after_it
#     not stop the chain
#   end an argument at a comma inside     an_argument_containing_a_comma_survives
#     a quote, which is the naive split
#   return from migrate_existing before   an_existing_hook_is_kept_and_still_runs
#     it keeps anything
#
# The sixth property, that the dispatcher reads the entry directory rather than
# discovering manifests when it fires, is structural rather than one line, so it
# has no mutation here and was not measured that way. What stands behind it is
# that its test carries both halves: the registered entry runs and the one that
# arrived afterwards does not, so a tool that registered nothing would fail it.
# =============================================================================

use test log

SHOOK="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/git-shook"

# A repository with git-shook not yet installed. Callers add manifests.
a_repo() {
    local w; w="$(mktemp -d)"
    git -C "$w" init -q .
    git -C "$w" config user.email t@example.com
    git -C "$w" config user.name t
    printf '%s' "$w"
}

# A manifest at <dir>, registering <name> to run <argv...> on pre-commit.
a_manifest_at() { # <repo> <dir-relative-or-empty> <name> <argv...>
    local w="$1" d="$2" name="$3"; shift 3
    local path="$w${d:+/$d}"
    mkdir -p "$path"
    {
        printf 'name = "%s"\n\n[hooks.pre-commit]\nrun = [' "$name"
        local first=1
        for a in "$@"; do
            [[ $first -eq 0 ]] && printf ', '
            # a quote inside an argument is escaped, the way a person writing
            # the manifest by hand would have to write it
            printf '"%s"' "${a//\"/\\\"}"
            first=0
        done
        printf ']\n'
    } > "$path/shook.toml"
}

commit_all() { # <repo> <message>
    git -C "$1" add -A
    git -C "$1" commit -q -m "$2" 2>&1
}

#[test]
two_manifests_both_register_on_one_event() {
    local w; w="$(a_repo)"
    a_manifest_at "$w" "" alpha sh -c 'echo alpha ran'
    a_manifest_at "$w" sub beta sh -c 'echo beta ran'
    commit_all "$w" init >/dev/null

    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    assert_ok test -f "$w/.shook/entries/pre-commit/alpha"
    assert_ok test -f "$w/.shook/entries/pre-commit/beta"
    rm -rf "$w"
}

#[test]
every_entry_runs_on_a_real_commit() {
    local w; w="$(a_repo)"
    a_manifest_at "$w" "" alpha sh -c 'echo ALPHA-RAN'
    a_manifest_at "$w" sub beta sh -c 'echo BETA-RAN'
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    printf 'x\n' > "$w/f.txt"
    local out; out="$(commit_all "$w" second)"

    assert_contains "$out" ALPHA-RAN
    assert_contains "$out" BETA-RAN
    rm -rf "$w"
}

#[test]
an_entry_runs_in_its_manifests_directory() {
    local w; w="$(a_repo)"
    a_manifest_at "$w" deep/inner beta sh -c 'echo WD-IS-$(basename "$(pwd)")'
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    printf 'x\n' > "$w/f.txt"
    local out; out="$(commit_all "$w" second)"

    assert_contains "$out" WD-IS-inner
    rm -rf "$w"
}

#[test]
entries_run_in_sorted_order() {
    local w; w="$(a_repo)"
    a_manifest_at "$w" one zzz sh -c 'echo THIRD'
    a_manifest_at "$w" two aaa sh -c 'echo FIRST'
    a_manifest_at "$w" three mmm sh -c 'echo SECOND'
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    printf 'x\n' > "$w/f.txt"
    local out; out="$(commit_all "$w" second)"
    local order; order="$(printf '%s\n' "$out" | grep -E '^(FIRST|SECOND|THIRD)$' | tr '\n' ' ')"

    assert_eq "$order" "FIRST SECOND THIRD "
    rm -rf "$w"
}

#[test]
a_failing_entry_refuses_the_commit() {
    local w; w="$(a_repo)"
    a_manifest_at "$w" "" aaa false
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    printf 'x\n' > "$w/f.txt"
    git -C "$w" add -A
    assert_fails git -C "$w" commit -q -m refused
    rm -rf "$w"
}

#[test]
a_failing_entry_stops_the_ones_after_it() {
    local w; w="$(a_repo)"
    a_manifest_at "$w" one aaa false
    a_manifest_at "$w" two zzz sh -c 'echo SHOULD-NOT-RUN'
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    printf 'x\n' > "$w/f.txt"
    git -C "$w" add -A
    local out; out="$(git -C "$w" commit -q -m refused 2>&1 || true)"

    assert_not_contains "$out" SHOULD-NOT-RUN
    rm -rf "$w"
}

#[test]
every_entry_sees_the_stdin_git_gave() {
    # `pre-push` hands refs over on stdin, so with one shared pipe only the
    # first entry would see them. The dispatcher replays a copy to each.
    local w; w="$(a_repo)"
    mkdir -p "$w/one" "$w/two"
    printf 'name = "aaa"\n\n[hooks.pre-commit]\nrun = ["sh", "-c", "echo ONE-SAW-$(cat)"]\n' > "$w/one/shook.toml"
    printf 'name = "zzz"\n\n[hooks.pre-commit]\nrun = ["sh", "-c", "echo TWO-SAW-$(cat)"]\n' > "$w/two/shook.toml"
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    local out; out="$(cd "$w" && printf 'REFS\n' | ./.shook/hooks/pre-commit 2>&1 || true)"

    assert_contains "$out" ONE-SAW-REFS
    assert_contains "$out" TWO-SAW-REFS
    rm -rf "$w"
}

#[test]
an_argument_containing_a_comma_survives() {
    # The manifest reader walks the run array character by character tracking
    # quotes. Splitting on commas would tear this one in two and neither half
    # would unquote.
    local w; w="$(a_repo)"
    a_manifest_at "$w" "" alpha sh -c 'echo GOT:$1' -- 'a,b'
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    local last; last="$(tail -n 1 "$w/.shook/entries/pre-commit/alpha")"

    assert_eq "$last" 'a,b'
    rm -rf "$w"
}

#[test]
an_argument_containing_a_space_stays_one_argument() {
    local w; w="$(a_repo)"
    a_manifest_at "$w" "" alpha sh -c 'echo x' 'two words'
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    local n; n="$(tail -n +2 "$w/.shook/entries/pre-commit/alpha" | wc -l | tr -d ' ')"

    assert_eq "$n" 4
    rm -rf "$w"
}

#[test]
an_existing_hook_is_kept_and_still_runs() {
    local w; w="$(a_repo)"
    mkdir -p "$w/.git/hooks"
    printf '#!/bin/sh\necho THE-OLD-HOOK\n' > "$w/.git/hooks/pre-commit"
    chmod +x "$w/.git/hooks/pre-commit"
    a_manifest_at "$w" "" alpha sh -c 'echo THE-NEW-ONE'
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    printf 'x\n' > "$w/f.txt"
    local out; out="$(commit_all "$w" second)"

    assert_contains "$out" THE-OLD-HOOK
    assert_contains "$out" THE-NEW-ONE
    rm -rf "$w"
}

#[test]
uninstall_puts_the_old_hook_back() {
    local w; w="$(a_repo)"
    mkdir -p "$w/.git/hooks"
    printf '#!/bin/sh\necho THE-OLD-HOOK\n' > "$w/.git/hooks/pre-commit"
    chmod +x "$w/.git/hooks/pre-commit"
    a_manifest_at "$w" "" alpha sh -c 'echo x'
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )
    ( cd "$w" && "$SHOOK" uninstall >/dev/null )

    assert_ok test -x "$w/.git/hooks/pre-commit"
    assert_contains "$(cat "$w/.git/hooks/pre-commit")" THE-OLD-HOOK
    assert_fails test -d "$w/.shook"
    rm -rf "$w"
}

#[test]
a_manifest_that_arrives_later_does_nothing_until_install_is_run_again() {
    # Discovery at hook time would mean a pull request adding a manifest
    # executes on the reviewer's machine as soon as they commit.
    local w; w="$(a_repo)"
    a_manifest_at "$w" "" alpha sh -c 'echo THE-KNOWN-ONE'
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    a_manifest_at "$w" hostile zzz sh -c 'echo THE-SMUGGLED-ONE'
    printf 'x\n' > "$w/f.txt"
    local out; out="$(commit_all "$w" second)"

    assert_contains "$out" THE-KNOWN-ONE
    assert_not_contains "$out" THE-SMUGGLED-ONE
    rm -rf "$w"
}

#[test]
an_untracked_manifest_is_not_discovered() {
    local w; w="$(a_repo)"
    a_manifest_at "$w" "" alpha sh -c 'echo x'
    commit_all "$w" init >/dev/null
    a_manifest_at "$w" scratch zzz sh -c 'echo SCRATCH'

    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    assert_fails test -f "$w/.shook/entries/pre-commit/zzz"
    rm -rf "$w"
}

#[test]
install_refuses_a_manifest_it_cannot_read() {
    local w; w="$(a_repo)"
    mkdir -p "$w"
    printf 'name = "alpha"\n\n[hooks.pre-commit]\nrun = ["sh", ohno]\n' > "$w/shook.toml"
    commit_all "$w" init >/dev/null

    assert_fails env -C "$w" "$SHOOK" install --yes
    rm -rf "$w"
}

#[test]
install_refuses_a_hooks_section_with_no_run() {
    local w; w="$(a_repo)"
    printf 'name = "alpha"\n\n[hooks.pre-commit]\n' > "$w/shook.toml"
    commit_all "$w" init >/dev/null

    assert_fails env -C "$w" "$SHOOK" install --yes
    rm -rf "$w"
}

#[test]
doctor_reports_a_manifest_that_was_never_registered() {
    local w; w="$(a_repo)"
    a_manifest_at "$w" "" alpha sh -c 'echo x'
    commit_all "$w" init >/dev/null

    local out; out="$(cd "$w" && "$SHOOK" doctor 2>&1 || true)"

    assert_contains "$out" "shook.toml"
    rm -rf "$w"
}

#[test]
doctor_is_quiet_once_everything_agrees() {
    local w; w="$(a_repo)"
    a_manifest_at "$w" "" alpha sh -c 'echo x'
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    local out; out="$(cd "$w" && "$SHOOK" doctor 2>&1)"

    assert_contains "$out" "all good"
    rm -rf "$w"
}

#[test]
a_name_that_would_escape_the_entry_directory_is_refused() {
    # Measured before the check existed: `name = "../../escaped"` put a file at
    # `.shook/escaped`, outside the entry directory entirely.
    local w; w="$(a_repo)"
    printf 'name = "../../escaped"\n\n[hooks.pre-commit]\nrun = ["true"]\n' > "$w/shook.toml"
    commit_all "$w" init >/dev/null

    assert_fails env -C "$w" "$SHOOK" install --yes
    assert_fails test -e "$w/.shook/escaped"
    rm -rf "$w"
}

#[test]
a_name_with_a_slash_is_refused() {
    local w; w="$(a_repo)"
    printf 'name = "a/b"\n\n[hooks.pre-commit]\nrun = ["true"]\n' > "$w/shook.toml"
    commit_all "$w" init >/dev/null

    assert_fails env -C "$w" "$SHOOK" install --yes
    rm -rf "$w"
}

#[test]
an_event_git_does_not_have_is_refused() {
    # doctor used to call such a manifest healthy, so nothing said the section
    # would never run.
    local w; w="$(a_repo)"
    printf 'name = "alpha"\n\n[hooks.pre-commmit]\nrun = ["true"]\n' > "$w/shook.toml"
    commit_all "$w" init >/dev/null

    assert_fails env -C "$w" "$SHOOK" install --yes
    rm -rf "$w"
}

#[test]
two_manifests_claiming_one_name_at_one_event_are_refused() {
    local w; w="$(a_repo)"
    a_manifest_at "$w" one same sh -c 'echo x'
    a_manifest_at "$w" two same sh -c 'echo y'
    commit_all "$w" init >/dev/null

    assert_fails env -C "$w" "$SHOOK" install --yes
    rm -rf "$w"
}

#[test]
an_argument_that_would_carry_a_newline_is_refused() {
    # The entry format is one argument per line, so a decoded \n would silently
    # become two arguments.
    local w; w="$(a_repo)"
    printf 'name = "alpha"\n\n[hooks.pre-commit]\nrun = ["sh", "-c", "a\\nb"]\n' > "$w/shook.toml"
    commit_all "$w" init >/dev/null

    assert_fails env -C "$w" "$SHOOK" install --yes
    rm -rf "$w"
}

#[test]
a_kept_hook_that_is_not_shell_still_runs() {
    # It was run through `sh` before, which turned a working python hook into a
    # syntax error while the tool reported it had been kept.
    local w; w="$(a_repo)"
    mkdir -p "$w/.git/hooks"
    printf '#!/usr/bin/env python3\nprint("PYTHON-HOOK-RAN")\n' > "$w/.git/hooks/pre-commit"
    chmod +x "$w/.git/hooks/pre-commit"
    a_manifest_at "$w" "" alpha sh -c 'echo x'
    commit_all "$w" init >/dev/null
    ( cd "$w" && "$SHOOK" install --yes >/dev/null )

    printf 'x\n' > "$w/f.txt"
    local out; out="$(commit_all "$w" second)"

    assert_contains "$out" PYTHON-HOOK-RAN
    rm -rf "$w"
}

#[test]
the_dispatcher_works_in_a_bare_repository() {
    # `rev-parse --show-toplevel` fails in a bare repository, so the dispatcher
    # exited 0 there and every push hook a bare repository exists to run was
    # silently inert.
    local b; b="$(mktemp -d)"
    git init -q --bare "$b"
    mkdir -p "$b/.shook/entries/pre-receive"
    printf '\n' > "$b/.shook/entries/pre-receive/aaa"
    printf 'sh\n-c\necho BARE-RAN\n' >> "$b/.shook/entries/pre-receive/aaa"
    mkdir -p "$b/.shook/hooks"
    "$SHOOK" dispatcher > "$b/.shook/hooks/pre-receive"
    chmod +x "$b/.shook/hooks/pre-receive"

    local out; out="$(cd "$b" && ./.shook/hooks/pre-receive </dev/null 2>&1 || true)"

    assert_contains "$out" BARE-RAN
    rm -rf "$b"
}

# `test_run` sources this file once per test, so the run block guards against
# starting a second suite inside the first.
if [[ -z "${_SHOOK_TEST_RUNNING:-}" ]]; then
    export _SHOOK_TEST_RUNNING=1
    test_run "${BASH_SOURCE[0]}"
    test_summary
    exit $?
fi

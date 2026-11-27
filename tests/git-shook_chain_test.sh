#!/usr/bin/env nutshell
# =============================================================================
# git-shook_chain_test.sh - the front on its base: the chain each event hands to
# shook
# =============================================================================
#
#   ./tests/git-shook_chain_test.sh
#
# Mutation-checked against a copy of `git-shook`, each mutation failing the
# test named beside it:
#
#   skip the copy of the base                the_dispatcher_runs_the_base_that_install_put_beside_it
#   take no notice of a missing base         a_dispatcher_with_no_base_refuses_and_says_what_to_do
#   skip the dispatcher comparison           doctor_names_a_dispatcher_another_git_shook_wrote
#   skip the base comparison                 doctor_names_a_base_that_differs_from_the_one_beside_it
#   install with no base found               install_with_no_base_anywhere_is_refused_before_anything_moves

use test log

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SHOOK="${GIT_SHOOK:-$HERE/git-shook}"

GIT_SCRUB=(env -u GIT_INDEX_FILE -u GIT_DIR -u GIT_WORK_TREE
    -u GIT_OBJECT_DIRECTORY -u GIT_COMMON_DIR -u GIT_ALTERNATE_OBJECT_DIRECTORIES)
unset GIT_INDEX_FILE GIT_DIR GIT_WORK_TREE \
    GIT_OBJECT_DIRECTORY GIT_COMMON_DIR GIT_ALTERNATE_OBJECT_DIRECTORIES

a_repo() {
    local w; w="$(mktemp -d)"
    "${GIT_SCRUB[@]}" git -C "$w" init -q .
    "${GIT_SCRUB[@]}" git -C "$w" config user.email t@example.com
    "${GIT_SCRUB[@]}" git -C "$w" config user.name t
    printf '%s' "$w"
}

# A clone-local manifest with the text given, installed.
installed_with() { # <repo> <manifest text>
    mkdir -p "$1/.git/shook/manifests"
    printf '%s\n' "$2" > "$1/.git/shook/manifests/mine.toml"
    ( cd "$1" && "$SHOOK" install --yes >/dev/null )
}

commit_of() { # <repo> <path> <message> -> the commit's output
    mkdir -p "$(dirname "$1/$2")"
    printf 'x\n' >> "$1/$2"
    "${GIT_SCRUB[@]}" git -C "$1" add -A
    "${GIT_SCRUB[@]}" git -C "$1" commit -q -m "$3" 2>&1
}

#[test]
the_dispatcher_runs_the_base_that_install_put_beside_it() {
    local w; w="$(a_repo)"
    installed_with "$w" 'name = "mine"

[hooks.pre-commit]
run = ["sh", "-c", "echo RAN"]'
    assert_ok cmp -s "$HERE/shook" "$w/.git/shook/shook"
    # a base that says it ran is the one the dispatcher called
    printf '#!/bin/sh\necho THE-BASE-RAN\n' > "$w/.git/shook/shook"
    assert_contains "$(commit_of "$w" f first)" THE-BASE-RAN
    rm -rf "$w"
}

#[test]
a_dispatcher_with_no_base_refuses_and_says_what_to_do() {
    local w; w="$(a_repo)"
    installed_with "$w" 'name = "mine"

[hooks.pre-commit]
run = ["true"]'
    rm "$w/.git/shook/shook"
    local out; out="$(commit_of "$w" f first || true)"
    assert_contains "$out" "the base is missing"
    assert_contains "$out" "git shook install"
    assert_fails "${GIT_SCRUB[@]}" git -C "$w" rev-parse -q --verify HEAD
    rm -rf "$w"
}

#[test]
install_with_no_base_anywhere_is_refused_before_anything_moves() {
    local w; w="$(a_repo)"
    local lone; lone="$(mktemp -d)"
    cp "$SHOOK" "$lone/git-shook"
    mkdir -p "$w/.git/shook/manifests"
    printf 'name = "mine"\n\n[hooks.pre-commit]\nrun = ["true"]\n' > "$w/.git/shook/manifests/mine.toml"
    local out; out="$(cd "$w" && PATH=/usr/bin:/bin "$lone/git-shook" install --yes 2>&1 || true)"
    assert_contains "$out" "runs its hooks on shook"
    assert_fails test -d "$w/.git/shook/entries"
    assert_fails git -C "$w" config --get core.hooksPath
    rm -rf "$w" "$lone"
}

#[test]
doctor_names_a_dispatcher_another_git_shook_wrote() {
    local w; w="$(a_repo)"
    installed_with "$w" 'name = "mine"

[hooks.pre-commit]
run = ["true"]'
    assert_contains "$(cd "$w" && "$SHOOK" doctor)" "all good"
    printf '#!/bin/sh\n# git-shook dispatcher v2\n' > "$w/.git/shook/hooks/pre-commit"
    chmod +x "$w/.git/shook/hooks/pre-commit"
    local out; out="$(cd "$w" && "$SHOOK" doctor || true)"
    assert_contains "$out" "the pre-commit dispatcher was written by another git-shook"
    rm -rf "$w"
}

#[test]
doctor_names_a_base_that_differs_from_the_one_beside_it() {
    local w; w="$(a_repo)"
    installed_with "$w" 'name = "mine"

[hooks.pre-commit]
run = ["true"]'
    printf '\n# changed\n' >> "$w/.git/shook/shook"
    assert_contains "$(cd "$w" && "$SHOOK" doctor || true)" "differs from"
    rm "$w/.git/shook/shook"
    assert_contains "$(cd "$w" && "$SHOOK" doctor || true)" "the base the dispatchers run on is missing"
    rm -rf "$w"
}

#[test]
uninstall_takes_the_base_with_it() {
    local w; w="$(a_repo)"
    installed_with "$w" 'name = "mine"

[hooks.pre-commit]
run = ["true"]'
    ( cd "$w" && "$SHOOK" uninstall >/dev/null )
    assert_fails test -e "$w/.git/shook/shook"
    rm -rf "$w"
}

if [[ -z "${_SHOOK_CHAIN_TEST_RUNNING:-}" ]]; then
    export _SHOOK_CHAIN_TEST_RUNNING=1
    test_run "${BASH_SOURCE[0]}"
    test_summary
    exit $?
fi

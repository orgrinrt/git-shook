#!/usr/bin/env nutshell
# =============================================================================
# git-shook_doctor_test.sh - what doctor checks entry by entry, and what
# `--only` leaves out
# =============================================================================
#
#   ./tests/git-shook_doctor_test.sh
#
# Mutation-checked against a copy of `git-shook`, each mutation failing the
# test named beside it:
#
#   skip the entry comparison                 an_entry_edited_by_hand_is_named_and_a_reinstall_puts_it_back
#   check every manifest under --only         only_leaves_out_a_manifest_it_was_not_asked_about
#   take any name under --only as found       only_a_name_no_manifest_answers_to_is_named

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
    printf '%s' "$w"
}

# A clone-local manifest named <name> with the text given.
a_manifest() { # <repo> <name> <text>
    mkdir -p "$1/.git/shook/manifests"
    printf '%s\n' "$3" > "$1/.git/shook/manifests/$2.toml"
}

installed() { # <repo>
    ( cd "$1" && "$SHOOK" install --yes >/dev/null )
}

#[test]
an_entry_edited_by_hand_is_named_and_a_reinstall_puts_it_back() {
    local w; w="$(a_repo)"
    a_manifest "$w" mine 'name = "mine"

[hooks.pre-commit]
run = ["true"]'
    installed "$w"
    assert_contains "$(cd "$w" && "$SHOOK" doctor)" "all good"
    printf '\nsh\n-c\necho tampered\n' > "$w/.git/shook/entries/pre-commit/mine"
    local out; out="$(cd "$w" && "$SHOOK" doctor || true)"
    assert_contains "$out" "the pre-commit entry of mine differs from what"
    assert_exits 1 sh -c "cd '$w' && '$SHOOK' doctor >/dev/null"
    installed "$w"
    assert_contains "$(cd "$w" && "$SHOOK" doctor)" "all good"
    rm -rf "$w"
}

#[test]
only_leaves_out_a_manifest_it_was_not_asked_about() {
    local w; w="$(a_repo)"
    a_manifest "$w" mine 'name = "mine"

[hooks.pre-commit]
run = ["true"]'
    ( cd "$w" && "$SHOOK" install --yes --only mine >/dev/null )
    # another manifest arrives and is not registered: the whole doctor fails on it
    a_manifest "$w" theirs 'name = "theirs"

[hooks.post-commit]
run = ["true"]'
    assert_contains "$(cd "$w" && "$SHOOK" doctor || true)" "declares [hooks.post-commit] and is not registered"
    # and the doctor of the one that was registered passes
    local out; out="$(cd "$w" && "$SHOOK" doctor --only mine)"
    assert_contains "$out" "all good"
    assert_not_contains "$out" "theirs"
    # while one of its own entries gone fails it, which is the control
    rm "$w/.git/shook/entries/pre-commit/mine"
    assert_contains "$(cd "$w" && "$SHOOK" doctor --only mine || true)" "declares [hooks.pre-commit] and is not registered"
    rm -rf "$w"
}

#[test]
only_a_name_no_manifest_answers_to_is_named() {
    local w; w="$(a_repo)"
    a_manifest "$w" mine 'name = "mine"

[hooks.pre-commit]
run = ["true"]'
    installed "$w"
    local out; out="$(cd "$w" && "$SHOOK" doctor --only nobody || true)"
    assert_contains "$out" "no manifest answers to nobody"
    assert_exits 1 sh -c "cd '$w' && '$SHOOK' doctor --only nobody >/dev/null"
    assert_exits 0 sh -c "cd '$w' && '$SHOOK' doctor --only mine >/dev/null"
    rm -rf "$w"
}

#[test]
a_flag_doctor_does_not_take_is_refused() {
    local w; w="$(a_repo)"
    a_manifest "$w" mine 'name = "mine"

[hooks.pre-commit]
run = ["true"]'
    installed "$w"
    assert_exits 1 sh -c "cd '$w' && '$SHOOK' doctor --no-such-flag 2>/dev/null"
    assert_contains "$(cd "$w" && "$SHOOK" doctor --no-such-flag 2>&1 || true)" "unknown flag"
    rm -rf "$w"
}

#[test]
only_still_names_a_hooks_path_that_is_not_git_shooks() {
    local w; w="$(a_repo)"
    a_manifest "$w" mine 'name = "mine"

[hooks.pre-commit]
run = ["true"]'
    installed "$w"
    git -C "$w" config core.hooksPath "$w/elsewhere"
    assert_contains "$(cd "$w" && "$SHOOK" doctor --only mine || true)" "so nothing here runs"
    rm -rf "$w"
}

if [[ -z "${_SHOOK_DOCTOR_TEST_RUNNING:-}" ]]; then
    export _SHOOK_DOCTOR_TEST_RUNNING=1
    test_run "${BASH_SOURCE[0]}"
    test_summary
    exit $?
fi

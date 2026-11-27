#!/usr/bin/env nutshell
# =============================================================================
# install_test.sh - what ./install places, with and without cargo
# =============================================================================
#
#   ./tests/install_test.sh

use test log

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

#[test]
without_cargo_git_shook_and_the_base_are_placed_and_the_front_is_said_left_out() {
    local p; p="$(mktemp -d)"
    local out; out="$(PATH=/usr/bin:/bin "$HERE/install" "$p" 2>&1)"
    assert_exits 0 test -x "$p/bin/git-shook"
    assert_exits 0 test -x "$p/bin/shook"
    assert_contains "$out" "cargo is not on PATH, so claude-shook, the Claude Code front, is not installed"
    assert_exits 1 test -e "$p/bin/claude-shook"
    rm -rf "$p"
}

if [[ -z "${_SHOOK_INSTALL_TEST_RUNNING:-}" ]]; then
    export _SHOOK_INSTALL_TEST_RUNNING=1
    test_run "${BASH_SOURCE[0]}"
    test_summary
    exit $?
fi

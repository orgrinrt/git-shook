#!/usr/bin/env nutshell
# =============================================================================
# install_test.sh - what ./install places
# =============================================================================
#
#   ./tests/install_test.sh

use test log

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

#[test]
install_places_git_shook_and_its_man_page() {
    local p; p="$(mktemp -d)"
    assert_exits 0 sh -c "'$HERE/install' '$p' >/dev/null"
    assert_exits 0 test -x "$p/bin/git-shook"
    assert_exits 0 test -f "$p/share/man/man1/git-shook.1"
    rm -rf "$p"
}

if [[ -z "${_SHOOK_INSTALL_TEST_RUNNING:-}" ]]; then
    export _SHOOK_INSTALL_TEST_RUNNING=1
    test_run "${BASH_SOURCE[0]}"
    test_summary
    exit $?
fi

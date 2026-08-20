#!/bin/sh
set -eu

stage_formula() {
    stage_formula_source=$1
    stage_formula_tap=$2
    stage_formula_tag=$3

    mkdir -p "${stage_formula_tap}/Formula"
    cp "${stage_formula_source}" "${stage_formula_tap}/Formula/quinjet.rb"
    git -C "${stage_formula_tap}" add -- Formula/quinjet.rb
    if git -C "${stage_formula_tap}" diff --cached --quiet -- Formula/quinjet.rb; then
        printf 'the tap already carries %s\n' "${stage_formula_tag}"
        return
    fi
    git -C "${stage_formula_tap}" config user.name 'github-actions[bot]'
    git -C "${stage_formula_tap}" config user.email '41898282+github-actions[bot]@users.noreply.github.com'
    git -C "${stage_formula_tap}" commit --quiet -m "Update the Quinjet formula to ${stage_formula_tag}"
}

selftest() {
    homebrew_tap_test_root=$(mktemp -d)
    trap 'rm -rf "${homebrew_tap_test_root}"' 0 1 2 15
    homebrew_tap_test_repository="${homebrew_tap_test_root}/tap"
    homebrew_tap_test_formula="${homebrew_tap_test_root}/quinjet.rb"

    git init --quiet --initial-branch=main "${homebrew_tap_test_repository}"
    printf 'tap\n' >"${homebrew_tap_test_repository}/README.md"
    git -C "${homebrew_tap_test_repository}" add -- README.md
    git -C "${homebrew_tap_test_repository}" \
        -c user.name=test -c user.email=test@example.com \
        commit --quiet -m 'Create the tap'

    printf 'first formula\n' >"${homebrew_tap_test_formula}"
    stage_formula "${homebrew_tap_test_formula}" "${homebrew_tap_test_repository}" v1.0.0
    homebrew_tap_test_content=$(
        git -C "${homebrew_tap_test_repository}" show HEAD:Formula/quinjet.rb
    )
    test "${homebrew_tap_test_content}" = 'first formula'

    homebrew_tap_test_first_commit=$(git -C "${homebrew_tap_test_repository}" rev-parse HEAD)
    stage_formula "${homebrew_tap_test_formula}" "${homebrew_tap_test_repository}" v1.0.0
    homebrew_tap_test_current_commit=$(
        git -C "${homebrew_tap_test_repository}" rev-parse HEAD
    )
    test "${homebrew_tap_test_current_commit}" = "${homebrew_tap_test_first_commit}"

    printf 'second formula\n' >"${homebrew_tap_test_formula}"
    stage_formula "${homebrew_tap_test_formula}" "${homebrew_tap_test_repository}" v1.0.1
    homebrew_tap_test_content=$(
        git -C "${homebrew_tap_test_repository}" show HEAD:Formula/quinjet.rb
    )
    test "${homebrew_tap_test_content}" = 'second formula'
    homebrew_tap_test_current_commit=$(
        git -C "${homebrew_tap_test_repository}" rev-parse HEAD
    )
    test "${homebrew_tap_test_current_commit}" != "${homebrew_tap_test_first_commit}"
    printf 'update_homebrew_tap: new, unchanged, and updated formulae work\n'
}

if [ "${1:-}" = '--selftest' ]; then
    selftest
    exit 0
fi

if [ "$#" -ne 3 ]; then
    printf 'usage: %s FORMULA TAP TAG\n' "$0" >&2
    exit 2
fi

stage_formula "$1" "$2" "$3"

#!/bin/sh
# shellcheck disable=SC2016,SC2310,SC2312 # the harness embeds scripts and reads substitutions on purpose

set -eu

ROOT=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd)
INSTALLER=${ROOT}/install.sh
TEST_ROOT=$(mktemp -d 2>/dev/null || mktemp -d -t quinjet-tests)
FIXTURES=${TEST_ROOT}/fixtures
FAKE_BIN=${TEST_ROOT}/bin
DOWNLOAD_LOG=${TEST_ROOT}/downloads.log
ORIGINAL_PATH=${PATH}
ROOT_INSTALLATION=

cleanup() {
    if [ -n "${ROOT_INSTALLATION}" ]; then
        rm -f /usr/local/bin/quinjet /usr/local/bin/q
    fi
    rm -rf "${TEST_ROOT}"
}
trap cleanup EXIT HUP INT TERM

mkdir -p "${FIXTURES}" "${FAKE_BIN}"
: >"${DOWNLOAD_LOG}"

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

assert_contains() {
    needle=$1
    file=$2
    grep -F "${needle}" "${file}" >/dev/null 2>&1 || fail "expected '${needle}' in ${file}"
}

assert_equals() {
    expected=$1
    actual=$2
    [ "${expected}" = "${actual}" ] || fail "expected '${expected}', got '${actual}'"
}

sha256() {
    file=$1
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "${file}" | awk '{print $1}'
    else
        shasum -a 256 "${file}" | awk '{print $1}'
    fi
}

prepare_release() {
    asset=$1
    contents=$2
    printf '%s' "${contents}" >"${FIXTURES}/${asset}"
    printf '%s  dist/%s\n' "$(sha256 "${FIXTURES}/${asset}")" "${asset}" >"${FIXTURES}/SHA256SUMS"
}

cat >"${FAKE_BIN}/uname" <<'EOF'
#!/bin/sh
case "${1:-}" in
    -s) printf '%s\n' "$QUINJET_TEST_OS" ;;
    -m) printf '%s\n' "$QUINJET_TEST_ARCH" ;;
    *) printf '%s %s\n' "$QUINJET_TEST_OS" "$QUINJET_TEST_ARCH" ;;
esac
EOF

cat >"${FAKE_BIN}/curl" <<'EOF'
#!/bin/sh
output=
url=
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o | --output)
            output=$2
            shift 2
            ;;
        http://* | https://*)
            url=$1
            shift
            ;;
        *)
            shift
            ;;
    esac
done
[ -n "$output" ] && [ -n "$url" ] || exit 2
printf '%s\n' "$url" >>"$QUINJET_TEST_DOWNLOAD_LOG"
cp "$QUINJET_TEST_FIXTURES/${url##*/}" "$output"
EOF
chmod +x "${FAKE_BIN}/uname" "${FAKE_BIN}/curl"

run_installer() {
    test_home=$1
    test_os=$2
    test_arch=$3
    shift 3
    mkdir -p "${test_home}"
    env \
        HOME="${test_home}" \
        PATH="${FAKE_BIN}:${ORIGINAL_PATH}" \
        SHELL="${QUINJET_TEST_SHELL-/bin/sh}" \
        QUINJET_TEST_OS="${test_os}" \
        QUINJET_TEST_ARCH="${test_arch}" \
        QUINJET_TEST_FIXTURES="${FIXTURES}" \
        QUINJET_TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
        QUINJET_NO_MODIFY_PATH=1 \
        sh "${INSTALLER}" "$@"
}

printf 'test: installs a pinned Linux x86_64 release\n'
case_dir=${TEST_ROOT}/linux-x86
bin_dir=${case_dir}/bin
prepare_release quinjet-linux-x86_64 'linux x86 binary'
run_installer "${case_dir}/home" Linux x86_64 --version 1.2.3 --bin-dir "${bin_dir}" >"${case_dir}.out" 2>&1
assert_equals 'linux x86 binary' "$(cat "${bin_dir}/quinjet")"
[ -x "${bin_dir}/quinjet" ] || fail "installed Unix binary is not executable"
assert_contains 'https://github.com/pulkitxm/quinjet/releases/download/v1.2.3/quinjet-linux-x86_64' "${DOWNLOAD_LOG}"
assert_contains 'verified SHA-256 checksum' "${case_dir}.out"

printf 'test: selects the latest macOS Apple Silicon release\n'
case_dir=${TEST_ROOT}/macos-arm
bin_dir=${case_dir}/bin
prepare_release quinjet-macos-aarch64 'macOS ARM binary'
run_installer "${case_dir}/home" Darwin arm64 --bin-dir "${bin_dir}" >"${case_dir}.out" 2>&1
assert_equals 'macOS ARM binary' "$(cat "${bin_dir}/quinjet")"
assert_contains 'https://github.com/pulkitxm/quinjet/releases/latest/download/quinjet-macos-aarch64' "${DOWNLOAD_LOG}"

printf 'test: selects the Linux ARM64 release\n'
case_dir=${TEST_ROOT}/linux-arm
bin_dir=${case_dir}/bin
prepare_release quinjet-linux-aarch64 'Linux ARM binary'
run_installer "${case_dir}/home" Linux aarch64 --bin-dir "${bin_dir}" >"${case_dir}.out" 2>&1
assert_equals 'Linux ARM binary' "$(cat "${bin_dir}/quinjet")"
assert_contains 'https://github.com/pulkitxm/quinjet/releases/latest/download/quinjet-linux-aarch64' "${DOWNLOAD_LOG}"

printf 'test: rejects a checksum mismatch without replacing an installation\n'
case_dir=${TEST_ROOT}/bad-checksum
bin_dir=${case_dir}/bin
mkdir -p "${bin_dir}"
printf 'existing binary' >"${bin_dir}/quinjet"
printf 'tampered binary' >"${FIXTURES}/quinjet-linux-x86_64"
printf '%064d  quinjet-linux-x86_64\n' 0 >"${FIXTURES}/SHA256SUMS"
if run_installer "${case_dir}/home" Linux x86_64 --bin-dir "${bin_dir}" >"${case_dir}.out" 2>&1; then
    fail "checksum mismatch unexpectedly succeeded"
fi
assert_equals 'existing binary' "$(cat "${bin_dir}/quinjet")"
assert_contains 'checksum verification failed' "${case_dir}.out"

printf 'test: installs completions for the configured shell\n'
case_dir=${TEST_ROOT}/completions
home_dir=${case_dir}/home
bin_dir=${case_dir}/bin
prepare_release quinjet-linux-x86_64 '#!/bin/sh
set -eu
[ "$1" = completions ] && [ "$2" = bash ] && [ "$3" = --install ] && [ "$4" = --automatic ]
target=${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion/completions/quinjet
mkdir -p "$(dirname "$target")"
printf "generated bash completions\\n" >"$target"'
QUINJET_TEST_SHELL=/bin/bash run_installer "${home_dir}" Linux x86_64 --bin-dir "${bin_dir}" >"${case_dir}.out" 2>&1
assert_contains 'generated bash completions' "${home_dir}/.local/share/bash-completion/completions/quinjet"
assert_contains 'installing bash completions' "${case_dir}.out"

printf 'test: installs q when SHELL is empty\n'
case_dir=${TEST_ROOT}/empty-shell
home_dir=${case_dir}/home
bin_dir=${case_dir}/bin
prepare_release quinjet-linux-x86_64 '#!/bin/sh
set -eu
[ "$1" = completions ] && [ "$2" = bash ] && [ "$3" = --install ] && [ "$4" = --automatic ]
target=${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion/completions/quinjet
mkdir -p "$(dirname "$target")"
printf "fallback bash completions\\n" >"$target"
ln -s quinjet "$(dirname "$0")/q"'
QUINJET_TEST_SHELL='' run_installer "${home_dir}" Linux x86_64 --bin-dir "${bin_dir}" >"${case_dir}.out" 2>&1
[ -L "${bin_dir}/q" ] || fail "q was not installed when SHELL was empty"
assert_contains 'fallback bash completions' "${home_dir}/.local/share/bash-completion/completions/quinjet"
assert_contains 'SHELL is not set; using bash completion paths' "${case_dir}.out"

printf 'test: updates a shell profile once for the default bin directory\n'
case_dir=${TEST_ROOT}/path-update
home_dir=${case_dir}/home
prepare_release quinjet-linux-x86_64 '#!/bin/sh
exit 0'
mkdir -p "${home_dir}"
for run in 1 2; do
    env \
        HOME="${home_dir}" \
        PATH="${FAKE_BIN}:${ORIGINAL_PATH}" \
        SHELL=/bin/zsh \
        QUINJET_TEST_OS=Linux \
        QUINJET_TEST_ARCH=x86_64 \
        QUINJET_TEST_FIXTURES="${FIXTURES}" \
        QUINJET_TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
        sh "${INSTALLER}" --bin-dir "${home_dir}/.local/bin" >"${case_dir}.${run}.out" 2>&1
done
assert_equals '1' "$(grep -c "^export PATH=\"\\\$HOME/.local/bin:\\\$PATH\"\$" "${home_dir}/.zshrc")"
assert_contains 'Added by the Quinjet installer' "${home_dir}/.zshrc"

printf 'test: rejects unsupported systems before downloading\n'
case_dir=${TEST_ROOT}/unsupported
before=$(wc -l <"${DOWNLOAD_LOG}" | tr -d ' ')
if run_installer "${case_dir}/home" FreeBSD x86_64 --bin-dir "${case_dir}/bin" >"${case_dir}.out" 2>&1; then
    fail "unsupported operating system unexpectedly succeeded"
fi
after=$(wc -l <"${DOWNLOAD_LOG}" | tr -d ' ')
assert_equals "${before}" "${after}"
assert_contains 'unsupported operating system: FreeBSD' "${case_dir}.out"

printf 'test: rejects unsafe version values before downloading\n'
case_dir=${TEST_ROOT}/invalid-version
before=$(wc -l <"${DOWNLOAD_LOG}" | tr -d ' ')
if run_installer "${case_dir}/home" Linux x86_64 --version 'v1/../../invalid' --bin-dir "${case_dir}/bin" >"${case_dir}.out" 2>&1; then
    fail "invalid release version unexpectedly succeeded"
fi
after=$(wc -l <"${DOWNLOAD_LOG}" | tr -d ' ')
assert_equals "${before}" "${after}"
assert_contains 'invalid release version' "${case_dir}.out"

if [ "${QUINJET_TEST_ROOT_DEFAULT:-0}" = 1 ]; then
    printf 'test: root installation is immediately available on PATH\n'
    assert_equals '0' "$(id -u)"
    [ ! -e /usr/local/bin/quinjet ] || fail "/usr/local/bin/quinjet already exists"
    [ ! -e /usr/local/bin/q ] || fail "/usr/local/bin/q already exists"
    case_dir=${TEST_ROOT}/root-default
    home_dir=${case_dir}/home
    prepare_release quinjet-linux-x86_64 '#!/bin/sh
set -eu
case "$1" in
    completions)
        [ "$2" = bash ] && [ "$3" = --install ] && [ "$4" = --automatic ]
        ln -s quinjet "$(dirname "$0")/q"
        ;;
    --version) printf "quinjet test\\n" ;;
    *) exit 2 ;;
esac'
    ROOT_INSTALLATION=1
    env \
        HOME="${home_dir}" \
        PATH="/usr/local/bin:${FAKE_BIN}:${ORIGINAL_PATH}" \
        SHELL= \
        QUINJET_TEST_OS=Linux \
        QUINJET_TEST_ARCH=x86_64 \
        QUINJET_TEST_FIXTURES="${FIXTURES}" \
        QUINJET_TEST_DOWNLOAD_LOG="${DOWNLOAD_LOG}" \
        sh "${INSTALLER}" >"${case_dir}.out" 2>&1
    assert_contains 'installed to /usr/local/bin/quinjet' "${case_dir}.out"
    assert_equals 'quinjet test' "$(PATH=/usr/local/bin quinjet --version)"
    assert_equals 'quinjet test' "$(PATH=/usr/local/bin q --version)"
fi

printf 'All shell installer tests passed.\n'

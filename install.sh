#!/bin/sh
# shellcheck disable=SC2310,SC2312 # the installer guards every command with || fail, which is exactly what these two audits flag
# Install Quinjet from GitHub Releases.

set -eu
umask 077

REPOSITORY="pulkitxm/quinjet"
RELEASES_URL="https://github.com/${REPOSITORY}/releases"
VERSION=${QUINJET_VERSION:-latest}
BIN_DIR=${QUINJET_INSTALL_DIR:-}
NO_MODIFY_PATH=${QUINJET_NO_MODIFY_PATH:-0}

info() {
    printf 'info: %s\n' "$*"
}

warn() {
    printf 'warning: %s\n' "$*" >&2
}

fail() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

has() {
    command -v "$1" >/dev/null 2>&1
}

usage() {
    # editorconfig-checker-disable
    cat <<'EOF'
Install the latest Quinjet release.

Usage: install.sh [OPTIONS]

Options:
  -v, --version VERSION  Release to install, such as v0.1.0 (default: latest)
  -b, --bin-dir DIR      Installation directory (default: $XDG_BIN_HOME or ~/.local/bin)
      --no-modify-path   Do not update a shell startup file
  -h, --help             Print this help

Environment variables:
  QUINJET_VERSION         Same as --version
  QUINJET_INSTALL_DIR     Same as --bin-dir
  QUINJET_NO_MODIFY_PATH  Set to 1 to avoid updating PATH
EOF
    # editorconfig-checker-enable
}

require_value() {
    option=$1
    count=$2
    [ "${count}" -ge 2 ] || fail "${option} requires a value"
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        -v | --version)
            require_value "$1" "$#"
            VERSION=$2
            shift 2
            ;;
        --version=*)
            VERSION=${1#*=}
            shift
            ;;
        -b | --bin-dir)
            require_value "$1" "$#"
            BIN_DIR=$2
            shift 2
            ;;
        --bin-dir=*)
            BIN_DIR=${1#*=}
            shift
            ;;
        --no-modify-path)
            NO_MODIFY_PATH=1
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            fail "unknown option: $1 (use --help for usage)"
            ;;
    esac
done

case "${NO_MODIFY_PATH}" in
    0 | false | no | '') NO_MODIFY_PATH=0 ;;
    1 | true | yes) NO_MODIFY_PATH=1 ;;
    *) fail "QUINJET_NO_MODIFY_PATH must be 0 or 1" ;;
esac

if [ -z "${BIN_DIR}" ]; then
    if [ -n "${XDG_BIN_HOME:-}" ]; then
        BIN_DIR=${XDG_BIN_HOME}
    else
        [ -n "${HOME:-}" ] || fail "HOME is not set; provide --bin-dir"
        BIN_DIR=${HOME}/.local/bin
    fi
fi
[ -n "${BIN_DIR}" ] || fail "the installation directory cannot be empty"

case "${VERSION}" in
    latest)
        RELEASE_URL=${RELEASES_URL}/latest/download
        VERSION_LABEL=latest
        ;;
    '')
        fail "the release version cannot be empty"
        ;;
    *)
        case "${VERSION}" in
            v*) RELEASE_TAG=${VERSION} ;;
            *) RELEASE_TAG=v${VERSION} ;;
        esac
        case "${RELEASE_TAG}" in
            v[0-9]*) ;;
            *) fail "invalid release version: ${VERSION}" ;;
        esac
        case "${RELEASE_TAG}" in
            *[!0-9A-Za-z._+-]*) fail "invalid release version: ${VERSION}" ;;
            *) ;;
        esac
        RELEASE_URL=${RELEASES_URL}/download/${RELEASE_TAG}
        VERSION_LABEL=${RELEASE_TAG}
        ;;
esac

OS=$(uname -s 2>/dev/null) || fail "could not identify the operating system"
ARCH=$(uname -m 2>/dev/null) || fail "could not identify the CPU architecture"

case "${OS}" in
    Darwin)
        if [ "${ARCH}" = x86_64 ] && has sysctl && [ "$(sysctl -in sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
            ARCH=arm64
            info "Rosetta detected; selecting the native Apple Silicon build"
        fi
        case "${ARCH}" in
            x86_64 | amd64) ASSET=quinjet-macos-x86_64 ;;
            arm64 | aarch64) ASSET=quinjet-macos-aarch64 ;;
            *) fail "Quinjet does not publish a macOS release for architecture '${ARCH}'" ;;
        esac
        BINARY_NAME=quinjet
        ;;
    Linux)
        case "${ARCH}" in
            x86_64 | amd64) ASSET=quinjet-linux-x86_64 ;;
            arm64 | aarch64) ASSET=quinjet-linux-aarch64 ;;
            *) fail "Quinjet does not publish a Linux release for architecture '${ARCH}'" ;;
        esac
        BINARY_NAME=quinjet
        ;;
    MINGW* | MSYS* | CYGWIN*)
        case "${ARCH}" in
            x86_64 | amd64) ASSET=quinjet-windows-x86_64.exe ;;
            *) fail "Quinjet does not publish a Windows release for architecture '${ARCH}'" ;;
        esac
        BINARY_NAME=quinjet.exe
        ;;
    *)
        fail "unsupported operating system: ${OS}"
        ;;
esac

if has curl; then
    DOWNLOADER=curl
elif has wget; then
    DOWNLOADER=wget
else
    fail "curl or wget is required to download Quinjet"
fi

if has sha256sum; then
    CHECKSUM_TOOL=sha256sum
elif has shasum; then
    CHECKSUM_TOOL=shasum
elif has openssl; then
    CHECKSUM_TOOL=openssl
else
    fail "sha256sum, shasum, or openssl is required to verify the download"
fi
has awk || fail "awk is required to verify the download"

make_temp_dir() {
    if TEMP_DIR=$(mktemp -d 2>/dev/null); then
        return
    fi
    TEMP_DIR=$(mktemp -d -t quinjet) || fail "could not create a temporary directory"
}

TEMP_DIR=
STAGED_BINARY=
cleanup() {
    if [ -n "${STAGED_BINARY}" ]; then
        rm -f "${STAGED_BINARY}" || true
    fi
    if [ -n "${TEMP_DIR}" ]; then
        rm -rf "${TEMP_DIR}" || true
    fi
}
trap cleanup EXIT HUP INT TERM
make_temp_dir

DOWNLOAD_PATH=${TEMP_DIR}/${ASSET}
CHECKSUMS_PATH=${TEMP_DIR}/SHA256SUMS

download() {
    url=$1
    destination=$2
    case "${DOWNLOADER}" in
        curl)
            curl --proto '=https' --tlsv1.2 -fsSL "${url}" -o "${destination}" ||
                fail "failed to download ${url}"
            ;;
        wget)
            wget -q "${url}" -O "${destination}" || fail "failed to download ${url}"
            ;;
        *) fail "no supported downloader is available" ;;
    esac
}

info "detected ${OS} ${ARCH}"
info "downloading Quinjet ${VERSION_LABEL}"
download "${RELEASE_URL}/SHA256SUMS" "${CHECKSUMS_PATH}"
download "${RELEASE_URL}/${ASSET}" "${DOWNLOAD_PATH}"

EXPECTED_CHECKSUM=$(awk -v asset="${ASSET}" '
    {
        name = $NF
        sub(/^\*/, "", name)
        sub(/^dist\//, "", name)
        if (name == asset) {
            print $1
            exit
        }
    }
' "${CHECKSUMS_PATH}")
case "${EXPECTED_CHECKSUM}" in
    '' | *[!0-9A-Fa-f]*) fail "the release checksum for ${ASSET} is missing or invalid" ;;
    *) ;;
esac
[ "${#EXPECTED_CHECKSUM}" -eq 64 ] || fail "the release checksum for ${ASSET} is missing or invalid"

case "${CHECKSUM_TOOL}" in
    sha256sum) ACTUAL_CHECKSUM=$(sha256sum "${DOWNLOAD_PATH}" | awk '{print $1}') ;;
    shasum) ACTUAL_CHECKSUM=$(shasum -a 256 "${DOWNLOAD_PATH}" | awk '{print $1}') ;;
    openssl) ACTUAL_CHECKSUM=$(openssl dgst -sha256 "${DOWNLOAD_PATH}" | awk '{print $NF}') ;;
    *) fail "no supported checksum tool is available" ;;
esac
EXPECTED_CHECKSUM=$(printf '%s' "${EXPECTED_CHECKSUM}" | tr '[:upper:]' '[:lower:]')
ACTUAL_CHECKSUM=$(printf '%s' "${ACTUAL_CHECKSUM}" | tr '[:upper:]' '[:lower:]')
[ "${EXPECTED_CHECKSUM}" = "${ACTUAL_CHECKSUM}" ] || fail "checksum verification failed for ${ASSET}"
info "verified SHA-256 checksum"

mkdir -p "${BIN_DIR}" || fail "could not create ${BIN_DIR}"
DESTINATION=${BIN_DIR}/${BINARY_NAME}
STAGED_BINARY=$(mktemp "${BIN_DIR}/.quinjet-install.XXXXXX") || fail "could not write to ${BIN_DIR}"
cp "${DOWNLOAD_PATH}" "${STAGED_BINARY}" || fail "could not write to ${BIN_DIR}"
chmod 755 "${STAGED_BINARY}" || fail "could not make the Quinjet binary executable"
mv -f "${STAGED_BINARY}" "${DESTINATION}" || fail "could not install Quinjet to ${DESTINATION}"
STAGED_BINARY=

install_completions() {
    configured_shell=${SHELL:-}
    shell_name=${configured_shell##*/}
    case "${shell_name}" in
        bash | elvish | fish | zsh)
            info "installing ${shell_name} completions"
            "${DESTINATION}" completions "${shell_name}" --install --automatic >/dev/null ||
                fail "could not install ${shell_name} completions"
            info "start a new ${shell_name} session to enable completions and q"
            ;;
        *) ;;
    esac
}

install_completions

path_contains_bin_dir() {
    case ":${PATH:-}:" in
        *:"${BIN_DIR}":*) return 0 ;;
        *) return 1 ;;
    esac
}

add_default_dir_to_path() {
    [ "${NO_MODIFY_PATH}" -eq 0 ] || return 1
    [ -n "${HOME:-}" ] || return 1
    [ "${BIN_DIR}" = "${HOME}/.local/bin" ] || return 1

    configured_shell=${SHELL:-}
    shell_name=${configured_shell##*/}
    dollar='$'
    case "${shell_name}" in
        fish)
            config_home=${XDG_CONFIG_HOME:-${HOME}/.config}
            profile=${config_home}/fish/config.fish
            path_line="fish_add_path \"${dollar}HOME/.local/bin\""
            ;;
        zsh)
            profile=${ZDOTDIR:-${HOME}}/.zshrc
            path_line="export PATH=\"${dollar}HOME/.local/bin:${dollar}PATH\""
            ;;
        bash)
            profile=${HOME}/.bashrc
            path_line="export PATH=\"${dollar}HOME/.local/bin:${dollar}PATH\""
            ;;
        *)
            profile=${HOME}/.profile
            path_line="export PATH=\"${dollar}HOME/.local/bin:${dollar}PATH\""
            ;;
    esac

    mkdir -p "$(dirname "${profile}")" || return 1
    if [ -f "${profile}" ] && grep -F "${path_line}" "${profile}" >/dev/null 2>&1; then
        return 0
    fi
    {
        printf '\n# Added by the Quinjet installer\n'
        printf '%s\n' "${path_line}"
    } >>"${profile}" || return 1
    info "added ${BIN_DIR} to PATH in ${profile}"
    return 0
}

printf '\nQuinjet was installed to %s\n' "${DESTINATION}"
if ! path_contains_bin_dir; then
    if add_default_dir_to_path; then
        warn "restart your shell or update PATH in the current shell before running quinjet"
    else
        warn "${BIN_DIR} is not on PATH"
        printf 'Run this before using Quinjet:\n  export PATH="%s:%sPATH"\n' "${BIN_DIR}" '$'
    fi
fi

if ! has git; then
    warn "Git is required at runtime but was not found on PATH"
    case "${OS}" in
        Darwin) warn "install Git with 'xcode-select --install' or your package manager" ;;
        Linux) warn "install Git with your system package manager" ;;
        *) warn "install Git from https://git-scm.com/downloads" ;;
    esac
fi

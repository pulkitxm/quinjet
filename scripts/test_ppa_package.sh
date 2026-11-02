#!/bin/sh

set -eu

if [ "$#" -ne 1 ]; then
    echo "usage: test_ppa_package.sh DSC_FILE" >&2
    exit 2
fi

work=$(mktemp -d)
dpkg-source -x "$1" "${work}/source"
sudo apt-get update
sudo apt-get install --yes --no-install-recommends devscripts equivs
(cd "${work}/source" && sudo mk-build-deps --install --remove \
    --tool 'apt-get --yes --no-install-recommends' debian/control)
(cd "${work}/source" && dpkg-buildpackage --build=binary -us -uc)

package=$(find "${work}" -maxdepth 1 -name 'quinjet_*.deb' -print | head -n 1)
dpkg-deb --info "${package}"
sudo apt-get install --yes "${package}"
quinjet --version
q --version
test -s /usr/share/bash-completion/completions/quinjet
test -s /usr/share/fish/vendor_completions.d/quinjet.fish
test -s /usr/share/zsh/vendor-completions/_quinjet
test -s /usr/share/man/man1/quinjet.1

repository=$(mktemp -d)
git -C "${repository}" init --quiet
printf 'hello\n' >"${repository}/README.md"
quinjet -C "${repository}" status | grep -F README.md

status=0
output=$(quinjet update 2>&1) || status=$?
printf '%s\n' "${output}"
test "${status}" -eq 4
printf '%s\n' "${output}" | grep -F 'sudo apt install --only-upgrade quinjet'

#!/bin/sh

set -eu

if [ "$#" -ne 1 ]; then
    echo "usage: test_snap.sh SNAP_FILE" >&2
    exit 2
fi

sudo snap install --dangerous --classic "$1"
snap list quinjet
/snap/bin/quinjet --version

completion=/usr/share/bash-completion/completions/quinjet
ls -l "${completion}"
completer=$(readlink "${completion}")
test "${completer##*/}" = complete.sh

repository=$(mktemp -d)
git -C "${repository}" init --quiet
printf 'hello\n' >"${repository}/README.md"
/snap/bin/quinjet -C "${repository}" status | grep -F README.md

status=0
output=$(/snap/bin/quinjet update 2>&1) || status=$?
printf '%s\n' "${output}"
test "${status}" -eq 4
printf '%s\n' "${output}" | grep -F 'sudo snap refresh quinjet'

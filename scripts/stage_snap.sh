#!/bin/sh

set -eu

if [ "$#" -ne 2 ]; then
    echo "usage: stage_snap.sh BINARY PAYLOAD_DIR" >&2
    exit 2
fi

binary=$1
payload=$2
completions="${payload}/share/bash-completion/completions"

rm -rf "${payload}"
install -D -m 0755 "${binary}" "${payload}/bin/quinjet"
install -d "${completions}"
"${payload}/bin/quinjet" completions bash >"${completions}/quinjet"
test -s "${completions}/quinjet"

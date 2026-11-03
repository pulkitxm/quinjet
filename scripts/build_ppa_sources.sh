#!/bin/sh

set -eu

if [ "$#" -lt 2 ]; then
    echo "usage: build_ppa_sources.sh VERSION OUTPUT_DIR [SERIES...]" >&2
    exit 2
fi

version=$1
output_dir=$2
shift 2

for command_name in cargo cargo-vendor-filterer dpkg-buildpackage git python3 tar xz; do
    command -v "${command_name}" >/dev/null 2>&1 || {
        echo "build_ppa_sources: ${command_name} is required" >&2
        exit 1
    }
done

root=$(git rev-parse --show-toplevel)
work=$(mktemp -d)
trap 'rm -rf "${work}"' EXIT
source_dir="${work}/quinjet-${version}"
orig="quinjet_${version}.orig.tar.xz"

git -C "${root}" archive --format=tar --prefix="quinjet-${version}/" HEAD | tar -x -C "${work}"
(cd "${root}" && cargo vendor-filterer \
    --platform=x86_64-unknown-linux-gnu \
    --platform=aarch64-unknown-linux-gnu \
    "${source_dir}/vendor" >/dev/null)
python3 - "${source_dir}/vendor" <<'PYTHON'
import json
import sys
from pathlib import Path

for checksum in Path(sys.argv[1]).glob("*/.cargo-checksum.json"):
    data = json.loads(checksum.read_text())
    data["files"] = {}
    checksum.write_text(json.dumps(data))
PYTHON
tar -C "${work}" --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner \
    -cf - "quinjet-${version}" | xz -9 -T1 >"${work}/${orig}"

if [ "$#" -eq 0 ]; then
    python3 "${root}/scripts/ppa_source.py" --list-series >"${work}/series"
else
    printf '%s\n' "$@" >"${work}/series"
fi

mkdir -p "${output_dir}"
cp "${work}/${orig}" "${output_dir}/"
while IFS= read -r series; do
    rm -rf "${source_dir}/debian"
    python3 "${root}/scripts/ppa_source.py" \
        --version "${version}" --series "${series}" --output "${source_dir}"
    (cd "${source_dir}" && dpkg-buildpackage --build=source -d -nc -sa -us -uc </dev/null)
    find "${work}" -maxdepth 1 -type f \
        \( -name '*.dsc' -o -name '*.debian.tar.xz' -o -name '*_source.changes' \
        -o -name '*_source.buildinfo' \) -exec mv {} "${output_dir}/" \;
done <"${work}/series"
ls -l "${output_dir}"

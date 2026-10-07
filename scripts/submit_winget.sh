#!/usr/bin/env bash
set -euo pipefail

tag=${1:-}
mode=${2:-}
if [[ "$#" -gt 2 ]] || { [[ -n "${mode}" ]] && [[ "${mode}" != --dry-run ]]; }; then
    printf 'usage: submit_winget.sh [vVERSION] [--dry-run]\n' >&2
    exit 1
fi

source_repo=pulkitxm/quinjet
fork=pulkitxm/winget-pkgs
upstream=microsoft/winget-pkgs
if [[ -z "${tag}" ]]; then
    tag=$(gh api "repos/${source_repo}/releases/latest" --jq .tag_name)
fi
if [[ ! ${tag} =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    printf 'not a stable release tag: %s\n' "${tag}" >&2
    exit 1
fi
version=${tag#v}
branch="new-pulkitxm-quinjet-${version}"
manifest_path="manifests/p/Pulkitxm/Quinjet/${version}"

work=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/quinjet-winget.XXXXXX")
trap 'rm -rf "${work}"' EXIT
gh api "repos/${source_repo}/releases/tags/${tag}" >"${work}/release.json"
jq -e '.draft == false and .prerelease == false' "${work}/release.json" >/dev/null
gh release download "${tag}" --repo "${source_repo}" --dir "${work}" \
    --pattern SHA256SUMS --pattern quinjet-winget-manifests.zip --pattern quinjet-windows-x86_64.zip

(
    cd "${work}"
    awk '$2 == "quinjet-winget-manifests.zip" || $2 == "quinjet-windows-x86_64.zip"' SHA256SUMS >checksums
    count=$(wc -l <checksums | tr -d ' ')
    test "${count}" = 2
    shasum -a 256 --check checksums
)
archive="${work}/quinjet-winget-manifests.zip"
expected=$(printf '%s\n' Pulkitxm.Quinjet.installer.yaml Pulkitxm.Quinjet.locale.en-US.yaml Pulkitxm.Quinjet.yaml | sort)
contents=$(unzip -Z1 "${archive}" | sort)
test "${contents}" = "${expected}"
mkdir "${work}/manifests"
for file in Pulkitxm.Quinjet.installer.yaml Pulkitxm.Quinjet.locale.en-US.yaml Pulkitxm.Quinjet.yaml; do
    unzip -p "${archive}" "${file}" >"${work}/manifests/${file}"
    grep -Fx 'PackageIdentifier: Pulkitxm.Quinjet' "${work}/manifests/${file}" >/dev/null
    grep -Fx "PackageVersion: \"${version}\"" "${work}/manifests/${file}" >/dev/null
done
checksum=$(awk '$2 == "quinjet-windows-x86_64.zip" {print toupper($1)}' "${work}/SHA256SUMS")
grep -Fx "    InstallerSha256: \"${checksum}\"" "${work}/manifests/Pulkitxm.Quinjet.installer.yaml" >/dev/null
grep -Fx "    InstallerUrl: https://github.com/${source_repo}/releases/download/${tag}/quinjet-windows-x86_64.zip" \
    "${work}/manifests/Pulkitxm.Quinjet.installer.yaml" >/dev/null

git clone --quiet --depth=1 --filter=blob:none --sparse "https://github.com/${upstream}.git" "${work}/repository"
git -C "${work}/repository" sparse-checkout set manifests/p/Pulkitxm/Quinjet
cd "${work}/repository"
if [[ -d "${manifest_path}" ]]; then
    printf 'WinGet already contains Pulkitxm.Quinjet %s\n' "${version}"
    exit 0
fi
existing=$(gh api "repos/${upstream}/pulls?head=pulkitxm:${branch}&state=open" --jq '.[0].html_url // empty')
if [[ -n "${existing}" ]]; then
    printf 'WinGet submission already open: %s\n' "${existing}"
    exit 0
fi

git remote set-url origin "https://github.com/${fork}.git"
if git ls-remote --exit-code origin "refs/heads/${branch}" >/dev/null; then
    git fetch --quiet --depth=1 origin "${branch}"
    git switch --quiet -c "${branch}" FETCH_HEAD
else
    sha=$(git rev-parse HEAD)
    if [[ "${mode}" = --dry-run ]]; then
        printf 'Would fast-forward %s master from %s\n' "${fork}" "${upstream}"
        pukbot ref create "refs/heads/${branch}" --repo "${fork}" --sha "${sha}" --dry-run --json
    else
        gh repo sync "${fork}" --source "${upstream}" --branch master
        pukbot ref create "refs/heads/${branch}" --repo "${fork}" --sha "${sha}" --json
    fi
    git switch --quiet -c "${branch}"
fi
mkdir -p "${manifest_path}"
cp "${work}/manifests/"*.yaml "${manifest_path}/"
git add "${manifest_path}"
if ! git diff --cached --quiet; then
    arguments=(commit create --repo "${fork}" --branch "${branch}" --message "chore: submit Quinjet ${version} to WinGet" --json)
    if [[ "${mode}" = --dry-run ]]; then arguments+=(--dry-run); fi
    pukbot "${arguments[@]}"
fi
jq -n --arg head "pulkitxm:${branch}" --arg version "${version}" \
    '{head: $head, base: "master", title: ("Update: Pulkitxm.Quinjet to " + $version),
        body: ("Updates Quinjet to " + $version + " using its released manifests and checksum-verified Windows archive.")}' \
    >"${work}/pull-request.json"
if [[ "${mode}" = --dry-run ]]; then
    cat "${work}/pull-request.json"
else
    gh api --method POST "repos/${upstream}/pulls" --input "${work}/pull-request.json" --jq .html_url
fi

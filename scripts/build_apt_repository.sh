#!/bin/sh

set -eu

if [ "$#" -ne 4 ]; then
    echo "usage: build_apt_repository.sh PACKAGES_DIR OUTPUT_DIR GPG_HOME FINGERPRINT" >&2
    exit 2
fi

packages_dir=$1
output_dir=$2
gpg_home=$3
fingerprint=$4

for command_name in apt-ftparchive dpkg-scanpackages gpg gzip; do
    command -v "${command_name}" >/dev/null 2>&1 || {
        echo "build_apt_repository: ${command_name} is required" >&2
        exit 1
    }
done

find "${packages_dir}" -maxdepth 1 -type f -name 'quinjet_*_*.deb' | grep -q . || {
    echo "build_apt_repository: no Quinjet packages found" >&2
    exit 1
}

install -d "${output_dir}/pool/main/q/quinjet"
find "${packages_dir}" -maxdepth 1 -type f -name 'quinjet_*_*.deb' \
    -exec cp {} "${output_dir}/pool/main/q/quinjet/" \;

for architecture in amd64 arm64; do
    packages_path="dists/stable/main/binary-${architecture}"
    install -d "${output_dir}/${packages_path}"
    (
        cd "${output_dir}"
        dpkg-scanpackages --arch "${architecture}" pool /dev/null \
            >"${packages_path}/Packages"
        gzip -9 -n -c "${packages_path}/Packages" >"${packages_path}/Packages.gz"
    )
done

(
    cd "${output_dir}"
    apt-ftparchive \
        -o APT::FTPArchive::Release::Origin=Quinjet \
        -o APT::FTPArchive::Release::Label=Quinjet \
        -o APT::FTPArchive::Release::Suite=stable \
        -o APT::FTPArchive::Release::Codename=stable \
        -o APT::FTPArchive::Release::Architectures='amd64 arm64' \
        -o APT::FTPArchive::Release::Components=main \
        -o APT::FTPArchive::Release::Description='Quinjet packages' \
        release dists/stable >dists/stable/Release
    gpg --batch --yes --homedir "${gpg_home}" --local-user "${fingerprint}" \
        --armor --detach-sign --output dists/stable/Release.gpg dists/stable/Release
    gpg --batch --yes --homedir "${gpg_home}" --local-user "${fingerprint}" \
        --armor --clearsign --output dists/stable/InRelease dists/stable/Release
    gpg --batch --homedir "${gpg_home}" --armor --export "${fingerprint}" \
        >quinjet.asc
)

echo "build_apt_repository: signed amd64 and arm64 repository"

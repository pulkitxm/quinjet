# apt

Quinjet publishes a signed apt repository for Debian and Ubuntu on x86-64 and
ARM64. Add its key and source once, then install the package:

```bash
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://quinjet.pulkit.page/apt/quinjet.asc | sudo tee /etc/apt/keyrings/quinjet.asc >/dev/null
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/quinjet.asc] https://quinjet.pulkit.page/apt stable main" | sudo tee /etc/apt/sources.list.d/quinjet.list >/dev/null
sudo apt update
sudo apt install quinjet
```

The signing key fingerprint is
`B7B5DF2C5A252DF24D2EA181C1336A9FF4ED3EAF`. Inspect the downloaded key before
installing it when you want to verify that value yourself:

```bash
curl -fsSL https://quinjet.pulkit.page/apt/quinjet.asc | gpg --show-keys --fingerprint
```

The package declares Git as a dependency and `gh` as a suggestion. It installs:

```text
/usr/bin/quinjet
/usr/bin/q
/usr/share/bash-completion/completions/quinjet
/usr/share/fish/vendor_completions.d/quinjet.fish
/usr/share/zsh/vendor-completions/_quinjet
/usr/share/man/man1/quinjet.1
```

## Update

```bash
sudo apt update
sudo apt install --only-upgrade quinjet
```

`quinjet update` refuses to replace an apt-owned executable because doing so
would make dpkg's installed-version record inaccurate. It prints the apt
upgrade command instead. `quinjet update --check` still checks the newest
release without changing the installation.

## Inspect

```bash
apt-cache policy quinjet
dpkg-query --show quinjet
dpkg-query --listfiles quinjet
```

## Remove

```bash
sudo apt remove quinjet
sudo rm /etc/apt/sources.list.d/quinjet.list
sudo rm /etc/apt/keyrings/quinjet.asc
sudo apt update
```

## Releasing

The Linux release jobs use `cargo-deb` to package the same static binaries that
the install script and Homebrew consume. Each package is installed and smoke
tested on its native release runner before upload. The successful release then
triggers the Pages workflow, which downloads the latest `.deb` files, generates
apt metadata for both architectures, signs `InRelease` and `Release.gpg`, and
asks apt to resolve and download Quinjet from the finished local repository
before deployment.

The private signing key lives only in the `APT_SIGNING_KEY` Actions secret. Its
public half is committed at `packaging/apt/quinjet.asc`, and the deployment
refuses to sign when their fingerprints differ.

# Security Policy

## Supported Versions

Quinjet is early-stage software. Security fixes are applied to the latest
published release and the `main` branch.

| Version | Supported |
| --- | --- |
| Latest release | Yes |
| `main` | Yes |
| Older releases | No |

## Reporting a Vulnerability

Do not open a public issue for a suspected vulnerability. Use GitHub's
[private vulnerability reporting form](https://github.com/pulkitxm/quinjet/security/advisories/new)
so the report and any follow-up remain confidential.

Include the affected version and platform, reproduction steps, expected and
observed behavior, potential impact, and any suggested mitigation. Remove
credentials, private repository contents, and personal data from the report.

The maintainer aims to acknowledge a complete report within three business
days and provide an initial assessment within seven business days. Remediation
and disclosure timing depend on severity, complexity, and release availability.
The reporter will receive updates when the assessment changes materially.

Please allow time for a fix and coordinated disclosure before publishing
details. A GitHub security advisory will be used to request a CVE when one is
warranted.

## Safe Harbor

Good-faith research that follows this policy is considered authorized. Avoid
privacy violations, service disruption, destructive testing, social
engineering, and access beyond what is necessary to demonstrate the issue.
Stop testing and report immediately if you encounter sensitive data. The
project will not pursue action against researchers who follow these rules and
make a reasonable effort to avoid harm.

## Security Model

Quinjet invokes installed `git` and `gh` executables directly with argument
arrays and never builds shell command strings from repository data. Git hooks,
credential helpers, signing programs, filters, and configuration still run
according to the repository and user's normal Git configuration. Only open
repositories you trust.

The loopback webhook listener treats request bodies as untrusted refresh
signals, caps headers and bodies, and refuses non-loopback peers. Updates are
downloaded from a fixed GitHub release, verified against the published SHA-256
checksum, and installed only after verification succeeds.

Reports are especially useful when they demonstrate command or argument
injection, writes outside the intended repository or application directories,
remote access to the webhook listener, credential exposure, verification
bypass, or unsafe behavior in a destructive operation.

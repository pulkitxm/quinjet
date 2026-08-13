# Security Policy

## Supported Versions

Quinjet is early-stage software. Security fixes are applied to the latest published version and the `main` branch.

## Reporting a Vulnerability

Please do not open a public issue for an exploitable vulnerability. Use GitHub's **Report a vulnerability** private security-advisory flow for this repository.

Include:

- The affected version and platform
- Reproduction steps or a minimal repository
- Expected and observed behavior
- Potential impact
- Any suggested mitigation

The maintainer will acknowledge a complete report as soon as practical, investigate it, and coordinate disclosure and a release when warranted.

## Security Model

Quinjet invokes the installed Git executable directly with argument arrays. It does not invoke Git through a shell. Git hooks, credential helpers, signing programs, filters, and configuration still run according to the repository and user's normal Git configuration; only open repositories you trust.

# Security Policy

We take security issues seriously, especially for functionality related to
wallets, key material, network access, and contract execution.

## Reporting a vulnerability

If you believe you have found a security vulnerability, please do not open a
public issue or discussion.

Please use GitHub's private vulnerability reporting flow for this
repository via the repository's **Security** tab, then **Advisories**, then
**Report a vulnerability** or use the direct link below:

https://github.com/ton-blockchain/acton/security/advisories/new

When possible, include:

- affected version, tag, or commit
- a short description of the impact
- reproduction steps or a proof of concept
- any relevant environment details

Please do not include real private keys, seed phrases, or other sensitive
credentials in the report.

## Supported versions

Security fixes are provided on a best-effort basis for:

- the latest stable release
- the current `master` branch / active development line

Older releases may not receive backported fixes. In most cases, users should
upgrade to the latest stable release.

## Public disclosure

Please do not disclose a vulnerability publicly until maintainers have had a
reasonable opportunity to investigate and ship or coordinate a fix.

When a report results in a user-relevant fix, the project should document it in
the changelog and, when appropriate, in a GitHub Security Advisory.

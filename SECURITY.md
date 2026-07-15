# Security Policy

## Supported Versions

This project is pre-1.0. Only the latest release on `main` is supported.

## Reporting a Vulnerability

Please **do not** open a public issue for security vulnerabilities.

Report privately via [GitHub Security
Advisories](https://github.com/sungurerdim/statusline/security/advisories/new)
for this repository. Include:

- A description of the vulnerability and its impact.
- Steps to reproduce (input JSON, config, or environment that triggers it).
- Affected version/commit.

You should get an initial response within a few days. Once a fix is
available, a new release will be cut and the advisory will be published.

## Scope

This is a local CLI tool that reads harness stdin JSON, local TOML config,
and shells out to `git` — it makes no network requests. Reports of interest
include: crashes/panics on malformed input, path traversal via config-derived
paths, or any way untrusted input could lead to command injection in the
`git` subprocess calls.

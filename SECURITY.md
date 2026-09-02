# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

seed-canvas is currently in pre-1.0 development. The latest minor release
receives security patches; older versions do not.

## Reporting a Vulnerability

**Please do not file public issues for security problems.**

Report vulnerabilities privately via
[GitHub's private security advisory](https://github.com/EZfan/seed-canvas/security/advisories/new)
for this repository. You should receive an acknowledgement within 72
hours. We will follow up with a detailed response and a coordinated
disclosure timeline.

Please include:

- A clear description of the issue and its impact.
- Reproduction steps (proof-of-concept code, screenshots, or a screen recording).
- Affected commit SHA, release tag, or branch.
- Your name / handle for the public advisory (optional; tell us if you prefer
  to remain anonymous).

We commit to:

- Confirming receipt within 72 hours.
- Providing a triage assessment within 7 days.
- Issuing a CVE and a patched release as soon as a fix is ready, crediting you
  (unless you prefer otherwise).

## Scope

The following are in scope:

- Code execution or arbitrary file write through crafted templates, seeds, or
  gallery imports.
- Cross-site scripting (XSS) in the bundled web viewer or embed widget.
- Determinism regressions that allow two seeds to produce visually identical
  outputs across platforms (these break the core security promise).
- Supply-chain risks: typosquatting in the public template registry.

The following are **not** in scope:

- Vulnerabilities in upstream dependencies that already have a published fix
  (please open a regular issue or PR instead).
- Reports based on theoretical attacks without a demonstrable exploit.

## Hall of Fame

We thank the following reporters (with their permission):

- _Be the first — your name here._
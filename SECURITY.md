# Security Policy

## Supported versions

This project is pre-release. Security fixes are applied to the latest `main` branch only.

## Reporting a vulnerability

Do **not** create a public GitHub issue for a suspected vulnerability involving session authentication, UDP response behaviour, rate limiting, source-address binding, data retention, cloud credentials, or infrastructure exposure.

Report it privately to the repository owner through GitHub’s private contact channel. Include:

- a concise description of the issue and impact;
- reproduction steps or a minimal proof of concept;
- affected commit/version and environment;
- any mitigation already tested.

Do not send credentials, production packet captures, customer data, internal addresses, or destructive proof-of-concept traffic.

## Response targets

- Acknowledge receipt: target within 7 days.
- Initial triage: target within 14 days.
- Disclosure: coordinated after a fix or mitigation is available.

These are best-effort targets for a personal project, not a support SLA.

## Safe testing boundary

Only test against local or explicitly project-controlled infrastructure. Never use this project to scan, probe, reflect traffic at, or test third-party production endpoints.

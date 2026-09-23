# Contributing

Thanks for contributing to Network Readiness Probe. This is a personal engineering POC focused on reliable, safe, bidirectional UDP diagnostics.

## Before opening an issue or pull request

1. Read [README.md](README.md), [ROADMAP.md](ROADMAP.md), and [AGENTS.md](AGENTS.md).
2. Check whether the proposed work belongs to the current milestone.
3. Never include customer, employer/vendor, SIP, RTP, packet-capture, credential, API-key, AWS-account, or production-endpoint data.
4. Do not add scanning, arbitrary UDP reflection, arbitrary target selection, or generic SIP service functionality.

## Development setup

Install the stable Rust toolchain through `rustup`, then run from the repository root:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo audit` is required in CI once the audit tool is configured.

## Development rules

- Follow the test-first workflow in [AGENTS.md](AGENTS.md): write a failing test, observe the expected failure, implement the minimum safe behavior, then run focused and workspace checks.
- Keep changes narrow and make one coherent change per pull request.
- Update documentation in the same change whenever protocol, API, metric, report, security, infrastructure, or user-visible behavior changes.
- Use conventional commits: `docs:`, `feat:`, `fix:`, `test:`, `security:`, `infra:`, or `chore:`.
- Use Rust’s standard library by default. Propose and justify a new dependency before adding it, including its security and operational impact.

## Required pull-request checks

Before requesting review:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

Include the actual command output or result in the pull-request description.

## Security-sensitive changes

Stop and request review before changing public ports, session/HMAC behaviour, session binding or expiry, rate limits, source-IP handling, IAM/Terraform, dependency class, or adding SIP/STUN/TURN/WebRTC.

Read [docs/threat-model.md](docs/threat-model.md) first. Document the threat, mitigation, tests, operational detection and rollback in the same PR.

## License

By contributing, you agree that your contribution is licensed under the [Apache License 2.0](LICENSE).

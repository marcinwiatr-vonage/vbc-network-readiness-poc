# AGENTS.md — Network Readiness Probe

This file governs human and AI-assisted development in this repository. It is intended for VS Code workflows using Vonage Codex access and any other coding agent.

## Mission

Build a small, secure, evidence-driven network diagnostic POC: a native endpoint agent and controlled regional UDP probe that measure bidirectional loss, jitter, RTT and throughput.

The project is a personal engineering POC. Do not represent it as a Vonage service or readiness test.

## Non-negotiable boundaries

1. **No Vonage/customer assets in this repository or runtime.** Do not use internal endpoints, credentials, logos, documents, customer identities, packet captures, SIP traces or non-public product information.
2. **No UDP reflection behavior.** The probe must not echo arbitrary data, accept arbitrary destinations or send a packet without a valid, short-lived, authenticated session bound to the observed source endpoint.
3. **No scanning.** Do not add port scanning, third-party endpoint probing, network discovery or arbitrary SIP/RTP testing.
4. **No fabricated diagnostics.** If UDP cannot establish, return an explicit unavailable result. Never invent loss, jitter, capacity, MOS or call-quality measurements.
5. **No MOS until assumptions are codified.** MOS/E-model requires documented codec, packetization, packet-loss model and delay assumptions, plus tests.
6. **No AWS click-ops.** All permanent cloud resources are Terraform-managed. Never run `terraform apply` without an explicit request and a reviewed plan.
7. **No secrets in code or chat.** Use `.env.example` with placeholders. Do not commit credentials, account IDs, private keys, HMAC masters or test tokens.

## Default implementation choices

- **Language:** Rust.
- **Endpoint agent:** Rust CLI; no Electron or browser dependency.
- **Probe:** Tokio-based Rust UDP daemon.
- **API:** Axum running on Tokio.
- **Infrastructure:** Terraform; single Frankfurt EC2 POC first.
- **Storage:** SQLite for local development; defer Postgres until required.
- **Containers:** Docker for API/probe; agent distributed as native binary.

Do not introduce frameworks, a database, Kubernetes, a message bus or extra cloud services without an issue/ADR explaining why the existing approach cannot satisfy the current milestone.

## Required engineering workflow

### Before implementation

1. Read `README.md`, `ROADMAP.md` and the task/issue completely.
2. Identify the smallest roadmap milestone affected.
3. State the acceptance criteria and test strategy before writing code.
4. For protocol, security, AWS or metric-semantic changes, add/update a document in `docs/` first.
5. Keep the implementation scope narrow. Do not opportunistically refactor unrelated code.

### Test-driven implementation

For every behavior change:

1. Write the failing unit or integration test.
2. Run it and verify it fails for the expected reason.
3. Implement the smallest safe solution.
4. Run focused tests, then the full suite and static checks.
5. Update docs and examples in the same change when behavior is user-visible.

Expected command set once the Rust workspace exists:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo audit
```

Never claim a change works if the relevant test/build was not actually run. Report tool failures directly.

## Protocol rules

- Use a versioned, fixed-header binary frame format.
- Include magic value, protocol version, test ID, direction, sequence number, monotonic timestamp, bounded payload length and HMAC.
- Reject malformed or oversized frames before variable memory allocation.
- Use CSPRNG-generated one-time IDs and short-lived per-test credentials.
- Bind session state to the first verified source IP:port. Do not accept a client-specified UDP destination.
- Enforce fixed maximum duration, payload, packet rate and concurrent sessions on both agent and probe.
- Use monotonic clocks for duration/inter-arrival calculations. Do not rely on wall-clock synchronization for jitter or RTT.
- Treat sequence number wraparound deliberately and test it before releasing a format.

## Metric rules

- **UL loss** is calculated only from the probe’s received sequence stream.
- **DL loss** is calculated only from the agent’s received sequence stream.
- Report both directions separately; never combine them into a single loss number.
- Report jitter as a defined calculation and percentile set, not an undefined average.
- Label RTT exactly as RTT; it is not one-way latency.
- A non-completed handshake is `UDP_UNREACHABLE_OR_BLOCKED`, not 100% loss.
- Any estimated capacity must name the exact traffic profile and assumptions.

## Security review triggers

Stop implementation and request review before proceeding when a change:

- opens an additional public port or changes ingress rules;
- affects session token/HMAC format, expiry, binding or rate limits;
- sends traffic to a client-provided target;
- stores source IP, persistent result history or any identifier;
- adds SIP, STUN/TURN, WebRTC or any protocol parser;
- adds third-party telemetry, analytics or external API calls;
- changes IAM permissions, Terraform state backend or CI secrets.

Document the threat, mitigation, test and operational rollback in `docs/threat-model.md` or an ADR.

## AWS rules

- Use `eu-central-1` only until Frankfurt POC exit criteria are met.
- Configure AWS Budget alerts before creation of public resources.
- Root MFA on; no root access keys.
- Use least privilege, encrypted storage and SSM instead of public SSH.
- Security group ingress must be exact and documented. Do not use `0.0.0.0/0` for SSH.
- Run `terraform fmt -check`, `terraform validate` and `terraform plan` before any apply.
- Never commit `terraform.tfstate`, `.tfvars` containing secrets, or AWS credentials.

## Documentation rules

- `README.md`: product boundary, architecture and developer entry point.
- `CONTRIBUTING.md`: contributor workflow and required checks.
- `SECURITY.md`: private security-reporting path and safe-testing boundary.
- `ROADMAP.md`: milestone sequence and acceptance criteria.
- `docs/architecture.md`: protocol/API/component contracts and ADRs.
- `docs/metrics.md`: exact metric definitions, units, assumptions and examples.
- `docs/report-contract.md`: required report fields, source of truth, persistence rules and status semantics.
- `docs/threat-model.md`: threats, controls, residual risks and abuse cases.
- Update the applicable document in every change that alters a contract, metric, security posture or infrastructure shape.

## Git and pull requests

- Branch names: `docs/...`, `feat/...`, `fix/...`, `security/...`, `infra/...`.
- Conventional commits: `docs:`, `feat:`, `fix:`, `test:`, `security:`, `infra:`, `chore:`.
- Keep one coherent change per PR. Do not mix refactors, feature work and infrastructure changes.
- PR description must include: purpose, scope, security impact, tests run, manual validation and rollback note when infrastructure changes.
- Do not merge a PR with failing checks or an unresolved security concern.

## Instructions for coding agents

- Do not use `git commit`, `git push`, GitHub API mutations, `terraform apply`, or AWS mutations unless the user explicitly requests that action in the current conversation.
- Do not silently install dependencies or change lockfiles. Explain and request approval if a new dependency materially changes the security or operational surface.
- Do not modify this `AGENTS.md` merely to bypass a constraint. Propose a reviewed change with rationale.
- Prefer standard library solutions for protocol, API and metric code unless a dependency demonstrably reduces risk.
- Return a concise summary with modified files and actual test output after each task.

## Definition of done

A task is done only when:

1. It meets the roadmap acceptance criterion.
2. Unit/integration tests cover normal and security/error paths.
3. `cargo test --workspace`, `cargo fmt --check`, and `cargo clippy --workspace --all-targets -- -D warnings` pass once Rust code exists.
4. Relevant docs are updated.
5. No secrets, customer/vendor data, unbounded UDP behavior or undocumented cloud resources were introduced.
6. The change is reviewed as a small, focused Git diff.

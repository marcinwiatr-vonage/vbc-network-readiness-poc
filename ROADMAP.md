# Delivery Roadmap

This roadmap is deliberately ordered by risk. We establish protocol correctness and abuse resistance locally before we introduce public cloud exposure, browser UI, load simulation, or SIP-related functionality.

## Engineering principles

- **Evidence first:** collect and show raw directional metrics before assigning a status.
- **Secure by construction:** unknown/expired sessions must receive no UDP response.
- **Local before cloud:** every network feature needs a deterministic localhost integration test.
- **Small vertical slices:** each milestone leaves a testable artifact, not a design-only layer.
- **No false precision:** do not add MOS or platform-readiness assertions without documented assumptions and validation.
- **Infrastructure as code:** all AWS resources are Terraform-managed; no click-ops.

## Milestone 0 — Repository and design baseline

**Objective:** establish the engineering contract before code.

- [x] Create README with boundaries, architecture, protocol and AWS approach.
- [x] Create `AGENTS.md` for VS Code / Codex contributors.
- [x] Create this roadmap.
- [x] Add Apache-2.0 `LICENSE` and `NOTICE` with the project copyright holder.
- [x] Add `CONTRIBUTING.md`, `SECURITY.md`, and issue/PR templates.
- [x] Add `docs/architecture.md`, `docs/metrics.md`, `docs/report-contract.md` and `docs/threat-model.md` as source-of-truth design docs.

**Exit criterion:** a new contributor can understand project scope, risks and first milestones without external context.

## Milestone 1 — Local protocol foundation

**Objective:** define and prove a safe UDP data plane on one machine.

### Deliverables

- [x] Rust workspace and `protocol` crate layout.
- [x] First authenticated fixed binary packet encode/decode path in `crates/protocol`.
- [x] Core frame fields: magic/version, test ID, direction, monotonic sequence/timestamp, bounded payload and HMAC-SHA-256 tag.
- [x] Initial round-trip test: authenticated 172-byte frame encodes and decodes unchanged.
- [ ] TDD negative-path codec tests: invalid HMAC, modified payload/tag, short frame, invalid magic/version/direction, oversized payload and declared-length mismatch.
- [ ] In-memory session registry with expiry and a single bound source address/port.
- [x] Minimal loopback UDP probe handler accepts one explicit known session and stays silent for an unknown test ID.
- [x] Local Rust integration test proves a valid `127.0.0.1` full-duplex exchange and verifies no response for an unknown session. Expired and invalid-HMAC silence tests remain pending.

### Required tests

```bash
cargo test -p protocol
cargo test -p probe
cargo test -p agent
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

**Exit criterion:** local agent and local probe exchange authenticated packets in both directions; no UDP response occurs for an unknown or expired test ID.

## Milestone 2 — Metric correctness and result contract

**Objective:** produce repeatable diagnostic output, including explicit unavailable-UDP handling.

### Deliverables

- [ ] `proto/result.schema.json`.
- [ ] Directional metric calculator: sent/received, loss, duplicates, reordering, jitter p50/p95, throughput.
- [ ] RTT calculator with min/mean/p95.
- [ ] JSON output from agent and server-side validation at result ingestion.
- [ ] Result fixtures: clean link, uplink loss, downlink loss, high jitter and no probe response.
- [ ] `docs/report-contract.md` acceptance tests: report all target capacity, directional media and controlled firewall fields with correct `not_tested`/`null` semantics.
- [ ] Agent cancellation and timeout paths.

### Mandatory semantics

- Uplink loss is calculated at the probe.
- Downlink loss is calculated at the agent.
- RTT is never described as one-way delay.
- Absence of a validated handshake returns `UDP_UNREACHABLE_OR_BLOCKED`; it does not return a loss percentage or MOS.

**Exit criterion:** fixture-based tests prove metrics and labels are correct for normal, asymmetric and failed connectivity paths.

## Milestone 3 — Secure local control plane

**Objective:** issue short-lived test credentials and persist minimal results.

### Deliverables

- [ ] `POST /v1/tests`: random test ID, expiry, selected region/probe, one-time HMAC material.
- [ ] `POST /v1/results`: schema, session and expiry validation.
- [ ] Per-source creation rate limit.
- [ ] Session-use and session-expiry tests.
- [ ] Local SQLite store with minimal fields and configured retention.
- [ ] Structured logs without packet payloads or customer identity.
- [ ] Health endpoint for API and probe.

**Exit criterion:** a local agent receives a session, completes a test and stores a valid result. Replayed/expired sessions are rejected without probe transmission.

## Milestone 4 — Build, packaging and CI baseline

**Objective:** make every change reproducible and gated.

### Deliverables

- [ ] Non-root Docker images for API and probe.
- [ ] `docker-compose.yml` for the local stack.
- [ ] GitHub Actions workflow running `cargo fmt --check`, Clippy with warnings denied, unit/integration tests, `cargo audit`, Docker build, and Terraform `fmt`/`validate` when Terraform exists.
- [ ] Cross-platform release builds for agent: Windows amd64, Linux amd64, macOS arm64/amd64 as appropriate.
- [ ] Version injected into agent output and API/probe health response.

**Exit criterion:** a clean checkout can run local tests and build artefacts; CI blocks regressions.

## Milestone 5 — Frankfurt AWS proof of concept

**Objective:** deploy exactly one controlled public probe.

### Deliverables

- [ ] AWS Budget and billing alarm configured manually before `terraform apply`.
- [ ] Terraform: VPC/networking only where required, EC2, Elastic IP, encrypted EBS, security group, IAM instance profile and CloudWatch logging.
- [ ] Security group exposes only required HTTPS and single UDP test port. Public SSH is disabled; use SSM.
- [ ] Manual deployment/runbook using versioned container image.
- [ ] External verification from a non-AWS network.
- [ ] Terraform destroy procedure and cost-control runbook.

### Acceptance checks

- [ ] Agent can test external endpoint → Frankfurt successfully.
- [ ] Unknown traffic gets no UDP response.
- [ ] Result stored by server matches local agent metrics within documented timing tolerance.
- [ ] `terraform plan` is clean after deployment.
- [ ] Destroying the environment removes all billable POC resources intentionally created by Terraform.

**Exit criterion:** a repeatable, bounded, externally verifiable Frankfurt test exists and can be torn down safely.

## Milestone 6 — Report and usability layer

**Objective:** present evidence cleanly without hiding diagnostic context.

### Deliverables

- [ ] Minimal report page by test ID.
- [ ] Display test timestamp, region, agent version, duration, UL/DL loss, p50/p95 jitter, RTT, throughput and warnings.
- [ ] Explicit condition labels: `UDP unreachable`, `uplink impairment`, `downlink impairment`, `variable latency`, `insufficient configured-profile capacity`.
- [ ] Raw values shown next to every condition label.
- [ ] Export JSON; HTML/PDF only after the data model is stable.

**Exit criterion:** a reader can identify direction and type of impairment without inferring hidden scoring rules.

## Milestone 7 — Controlled load profile

**Objective:** estimate capacity for a documented, configurable traffic profile.

- [ ] Define codec-like profile assumptions (payload, packetization interval, transport overhead and aggregate rate).
- [ ] Add rate ramp with firm ceilings and cancellation.
- [ ] Detect degradation using documented p95 jitter/loss criteria.
- [ ] Report **estimated supported calls for the configured profile** only.
- [ ] Validate with controlled packet loss/jitter scenarios.

**Exit criterion:** output is technically reproducible and does not claim generic or vendor-specific call capacity.

## Milestone 8 — Second region and observability

**Objective:** add comparative network-path evidence.

- [ ] Add London probe only after Frankfurt is stable.
- [ ] Make region selection explicit.
- [ ] Add aggregate health metrics and alerts for the project’s own probes.
- [ ] Add test-result retention and deletion policy.
- [ ] Compare same endpoint/profile across regions without treating region selection as a platform-route assertion.

## Milestone 9 — Optional SIP ALG marker test

**Objective:** detect controlled payload rewriting, not provide a SIP platform.

This milestone requires a dedicated threat-model review and is blocked until all earlier milestones are complete.

- [ ] Design a one-time, controlled SIP `OPTIONS` marker test with unique `Via`, `Contact` and SDP values.
- [ ] Compare sent and probe-received payload precisely.
- [ ] Report field-level mutation only.
- [ ] Never accept real SIP registration credentials or target production platforms.
- [ ] Add explicit security tests and rate limits.

## Out-of-scope backlog

- Native GUI/Electron client.
- Continuous endpoint monitoring.
- Automated remediation suggestions.
- Customer or tenant integration.
- VBC/VCC/Vonage-product integration.
- Public multi-tenant hosting.

These remain out of scope until the core project has a documented product owner, security model and operating model.

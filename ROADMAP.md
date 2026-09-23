# Delivery Roadmap

This roadmap is ordered by risk: protocol correctness and abuse resistance precede public cloud exposure, report presentation, load profiling, or any SIP-related work.

## Engineering principles

- **Evidence first:** collect and show directional raw metrics before assigning a condition.
- **Secure by construction:** unknown, expired, replayed, or invalidly authenticated traffic receives no UDP response.
- **Local before cloud:** each network feature has a deterministic loopback integration test.
- **Small vertical slices:** every milestone leaves a testable artifact.
- **No false precision:** no MOS or generic capacity claim without documented assumptions and validation.
- **Infrastructure as code:** permanent AWS resources are Terraform-managed.

## Milestone 0 — Repository and design baseline

**Objective:** establish the engineering contract before code.

- [x] Create README with scope, architecture, protocol, and AWS approach.
- [x] Create contributor, security, and project-governance documentation.
- [x] Add architecture, metrics, report-contract, and threat-model source-of-truth documents.

**Exit criterion:** a new contributor can understand scope, risks, protocol, and delivery sequence without external context.

## Milestone 1 — Local protocol foundation

**Objective:** define and prove a safe UDP data plane on one machine.

### Deliverables

- [x] Rust workspace and `protocol` crate layout.
- [x] Canonical fixed-header v2 frame codec: magic `[0x4E, 0x52, 0x50, 0x02]`, version at offset 4, 16-byte ID at offsets 5–20, direction at 21, sequence at 22–25, monotonic timestamp at 26–33, payload length at 34–35, payload, then HMAC-SHA256. The retired experimental v1 layout is explicitly unsupported.
- [x] Frame bounds: 68-byte minimum, 1268-byte maximum, and 1200-byte payload ceiling.
- [x] Round-trip test proving authenticated binary encoding/decoding.
- [ ] TDD negative-path tests for invalid HMAC, modified payload/tag, short frame, invalid magic/version/direction, oversized payload, and declared-length mismatch.
- [ ] In-memory session registry with expiry and a single bound source address/port.
- [x] Loopback probe handler accepts an explicit known session and stays silent for an unknown test ID.
- [x] Loopback integration test proves a valid full-duplex exchange and no response for an unknown session.

### Required tests

```bash
cargo test -p protocol
cargo test -p probe
cargo test -p agent
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

**Exit criterion:** local agent and probe exchange authenticated frames in both directions; unknown, expired, and invalid-HMAC frames receive no response.

## Milestone 2 — Metric correctness and result contract

**Objective:** produce repeatable diagnostic output, including explicit unavailable-UDP handling.

### Deliverables

- [ ] `proto/result.schema.json`.
- [ ] Directional metric calculator: sent/received, loss, duplicates, reordering, jitter p50/p95, and throughput.
- [ ] RTT calculator with min/mean/p95.
- [ ] JSON output from agent and server-side validation at result ingestion.
- [ ] Result fixtures for clean link, directional loss, high jitter, and no probe response.
- [ ] Contract tests for `null` metric values and `not_tested` fields where measurement did not occur.
- [ ] Agent cancellation and timeout paths.

### Mandatory semantics

- Uplink loss is calculated only from the probe's received sequence stream.
- Downlink loss is calculated only from the agent's received sequence stream.
- RTT is never described as one-way latency.
- Missing validated handshake returns `UDP_UNREACHABLE_OR_BLOCKED`, not a loss percentage or MOS.

**Exit criterion:** fixture-based tests prove labels and metrics for normal, asymmetric, and failed-connectivity paths.

## Milestone 3 — Secure local control plane

**Objective:** issue short-lived credentials and persist minimal results.

- [ ] `POST /v1/tests`: CSPRNG test ID, expiry, selected probe, exact UDP port, and one-time HMAC material.
- [ ] `POST /v1/results`: schema, session, expiry, range, and duplicate-submission validation.
- [ ] Per-source session-creation rate limit.
- [ ] Session-use and session-expiry tests.
- [ ] Local SQLite result store with configured retention.
- [ ] Structured logs without payloads or customer identity.
- [ ] Health endpoints for API and probe.

**Exit criterion:** a local agent receives a session, completes a test, and stores one valid aggregate result; replayed or expired sessions are rejected without probe transmission.

## Milestone 4 — Build, packaging, and CI baseline

**Objective:** make every change reproducible and gated.

- [ ] Non-root Docker images for API and probe.
- [ ] Local compose configuration.
- [ ] CI running format, lint, unit/integration tests, dependency audit, container build, and Terraform validation when Terraform exists.
- [ ] Cross-platform agent release builds.
- [ ] Version injected into agent output and API/probe health responses.

**Exit criterion:** a clean checkout can run local tests and build artifacts; CI blocks regressions.

## Milestone 5 — Frankfurt AWS proof of concept

**Objective:** deploy exactly one controlled public probe in `fra` / `eu-central-1`.

### Deliverables

- [ ] AWS Budget and billing alarm configured manually before any apply.
- [ ] Terraform for only the required EC2, Elastic IP, encrypted storage, security group, IAM profile, and logging.
- [ ] Security group exposes required HTTPS plus exactly one UDP listener: `10000` or `16384`; public SSH is disabled and SSM is used.
- [ ] Deployment/runbook records the selected listener port, probe ID, binary/image version, rollback, and destroy procedure.
- [ ] External verification from a non-AWS network uses a provisioned one-time session and confirms that random/unknown UDP receives no response.
- [ ] Terraform destroy procedure and cost-control runbook.

### Acceptance checks

- [ ] Agent completes a Frankfurt test through the configured UDP port.
- [ ] Unknown, expired, replayed, and invalid-HMAC traffic receives no UDP response.
- [ ] Stored server result and agent result agree within documented timing tolerance while retaining separate directional authority.
- [ ] `terraform plan` is clean after deployment.
- [ ] Destroy removes intentionally created billable POC resources.

**Exit criterion:** a repeatable, bounded Frankfurt test exists and can be safely torn down.

## Milestone 6 — Report and usability layer

**Objective:** present evidence without hiding diagnostic context.

- [ ] Minimal report page by test ID.
- [ ] Display test timestamp, selected probe ID/region, protocol/report schema versions, agent version, duration, UL/DL loss, p50/p95 jitter, RTT, throughput, warnings, and status.
- [ ] Render unavailable or omitted measurements as `null`/`not_tested`, never as zero or an invented result.
- [ ] Show the actual controlled UDP port and reachability observation; do not present a port matrix as a scanner.
- [ ] Condition labels (`UDP unreachable`, `uplink impairment`, `downlink impairment`, `variable latency`, `insufficient configured-profile capacity`) always appear beside raw values and their threshold version.
- [ ] Export JSON; HTML/PDF only after schema stability.

**Exit criterion:** a reader can identify the measured direction, source of truth, selected endpoint, and reason a value is unavailable without inferring hidden scoring rules.

## Milestone 7 — Controlled load profile

**Objective:** estimate capacity only for a documented configurable traffic profile.

- [ ] Define payload, packetization interval, transport-overhead assumptions, aggregate rate, and duration.
- [ ] Add a rate ramp with firm ceilings and cancellation.
- [ ] Detect degradation using documented p95 jitter/loss criteria.
- [ ] Label output **estimated supported calls for the configured profile** only.
- [ ] Validate with controlled packet-loss and jitter scenarios.

**Exit criterion:** output is reproducible and does not claim generic capacity.

## Milestone 8 — Additional regions and observability

**Objective:** add comparative project-controlled path evidence only after Frankfurt is stable.

- [ ] Add regions one at a time: London (`lon` / `eu-west-2`), Virginia (`va` / `us-east-1`), Oregon (`or` / `us-west-2`), Singapore (`sg` / `ap-southeast-1`), and Sydney (`sy` / `ap-southeast-2`).
- [ ] Give every region an explicit probe ID, exact listener port (`10000` or `16384`), Terraform root, health checks, cost budget, rollback, and destroy procedure.
- [ ] Make selection explicit in the session and report; region choice is not a claim about any production route.
- [ ] Add aggregate health metrics and alerts for project probes without logging packet payloads.
- [ ] Implement tested result-retention and deletion policy before retaining cross-region history.
- [ ] Compare the same endpoint/profile across regions only when test versions and profile parameters match.

**Exit criterion:** each added region is independently deployable, observable, disposable, and clearly identified in its results.

## Milestone 9 — Optional controlled SIP marker test

**Objective:** detect controlled payload rewriting without providing a general SIP service.

This milestone is blocked until all earlier milestones are complete and a dedicated threat-model review approves the exact protocol, listener, limits, and rollback.

- [ ] Define one-time isolated marker traffic with unique values and strict parsing/size/rate ceilings.
- [ ] Bind the test to an authenticated, short-lived session and project-controlled endpoint; do not accept credentials, arbitrary destinations, registrations, or production targets.
- [ ] Compare sent and received marker bytes and report only field-level mutation plus `not_tested`/unavailable reasons.
- [ ] Require parser fuzz/error-path tests, no-response tests for unknown/expired/replayed sessions, ingress review, monitoring, and an immediate disable/destroy procedure.
- [ ] Keep any optional marker listener separate from the UDP readiness listener and document its exact purpose and port before Terraform exposure.

**Exit criterion:** the isolated test can report controlled mutation evidence without acting as an open relay, scanner, registration service, or general-purpose endpoint.

## Out-of-scope backlog

- Native GUI client.
- Continuous endpoint monitoring.
- Automated remediation suggestions.
- Customer or tenant integration.
- Public multi-tenant hosting.

These remain out of scope until the core project has a documented product owner, security model, and operating model.

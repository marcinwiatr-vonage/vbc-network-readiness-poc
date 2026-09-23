# Network Readiness Probe

> A personal engineering proof of concept for measuring **bidirectional UDP network quality** between an endpoint and a regional probe.

[![Status: design](https://img.shields.io/badge/status-design-blue)](#project-status)
[![Language: Rust](https://img.shields.io/badge/language-Rust-DEA584)](#technology-decisions)
[![Infrastructure: Terraform](https://img.shields.io/badge/infrastructure-Terraform-7B42BC)](#aws-poc-boundary)

## Important boundary

This repository is an **independent personal project**. It is not affiliated with, endorsed by, or intended to replace any Vonage product or official readiness test. It must not use Vonage branding, customer data, internal endpoints, SIP credentials, packet captures, or call content.

Results are point-in-time measurements between an endpoint and this project’s selected probe. They do not certify a network, validate a third-party platform, guarantee call quality, or replace production-path diagnostics.

## Problem statement

HTTP/TCP speed tests can show bandwidth and a basic latency result, but they do not provide the evidence needed to diagnose real-time media degradation. Voice and real-time application quality is often determined by conditions that are directional and time-sensitive:

- UDP reachability;
- uplink and downlink loss measured independently;
- loss bursts and out-of-order packets;
- jitter distribution, not a single average;
- RTT under a defined traffic profile;
- sustained usable throughput rather than a brief HTTP peak.

Network Readiness Probe will run a **bounded, authenticated, full-duplex UDP test** and return support-useful raw evidence alongside carefully scoped interpretation.

## Goals

- Build a cross-platform endpoint agent in Rust for Windows, Linux, and macOS.
- Operate a controlled regional UDP probe, beginning in Frankfurt (`eu-central-1`).
- Measure UL/DL packet loss, jitter p50/p95, RTT and achieved throughput separately.
- Produce a VoIP-style report with runner-visible environment metadata, target profile, capacity, directional media metrics and controlled firewall reachability.
- Make test execution safe: short-lived session credentials, strict rate limits, fixed duration and no UDP reflection behavior.
- Define all cloud infrastructure in Terraform and make builds/deployment repeatable.
- Keep the first release focused on diagnostic integrity rather than a polished dashboard.

## Non-goals for the MVP

- Public production service or customer-support workflow.
- Claims of vendor- or platform-specific readiness.
- Third-party SBC, SIP, RTP, media-server, or port scanning.
- SIP registration or collection of credentials.
- A generic UDP echo service.
- A MOS score before codec, packetization, impairment model and one-way-delay assumptions are explicitly documented.
- Electron desktop application; the agent should remain a small native Rust binary.

## Architecture

```text
+-------------------------------------+
| Endpoint agent (Rust CLI)           |
|-------------------------------------|
| - requests a one-time test session  |
| - validates and sends UDP frames    |
| - receives probe traffic            |
| - calculates local DL metrics       |
+----------------+--------------------+
                 | HTTPS: session + result
                 | UDP: bounded packet train
                 v
+-------------------------------------+       +------------------------------------+
| API / session service               |<----->| Regional UDP probe                 |
|-------------------------------------|       |------------------------------------|
| - test ID and HMAC issue            |       | - active-session registry          |
| - expiry and rate limiting          |       | - controlled reverse UDP stream    |
| - result schema validation          |       | - calculates server-side UL stats  |
+----------------+--------------------+       +----------------+-------------------+
                 |                                             |
                 +---------------- AWS eu-central-1 -----------+
```

Two observers are deliberate. The probe establishes whether agent-originated packets arrived (**uplink**), while the endpoint establishes what it received from the probe (**downlink**). Neither direction is inferred from one observer alone.

## Measurement model

### Test lifecycle

1. The agent calls `POST /v1/tests` over HTTPS.
2. The API rate-limits the request and returns a random, single-use `test_id`, expiry, UDP target and per-test HMAC key.
3. The agent sends an authenticated UDP hello to the selected probe.
4. The probe starts its reverse stream only if the session is valid. It binds the session to the observed source address and port.
5. Agent and probe exchange packets for a fixed duration and bounded rate.
6. The agent posts its result to `POST /v1/results`; the API verifies test ID, expiry and result schema.

### UDP frame

The wire format must be fixed-length-header binary framing, not JSON per datagram:

```text
magic | protocol_version | direction | test_id | sequence_number |
monotonic_send_timestamp | payload_length | random_payload | HMAC
```

The protocol implementation must reject malformed datagrams before allocating variable-sized payloads, cap payload length, and delete session state after expiry.

### Reported metrics

| Metric | Source and definition |
|---|---|
| Uplink loss | Missing authenticated sequence numbers as observed by the probe. |
| Downlink loss | Missing authenticated sequence numbers as observed by the agent. |
| Jitter | Packet inter-arrival variation per receiving side; report p50 and p95 for both directions. |
| RTT | Acknowledged timestamp round trip; report min, mean and p95. It is not one-way latency. |
| Throughput | Received payload bytes over the measured interval, independently for UL and DL. |
| Reachability | Explicit `UDP_UNREACHABLE_OR_BLOCKED` result when a valid handshake cannot complete. |

A test must never fabricate quality metrics after failed UDP connectivity.

## AWS POC boundary

The project starts in a personal AWS account and is suitable only for private development and limited proof-of-concept use.

Before any public resource is created:

1. Configure an AWS Budget with low monthly thresholds and billing alerts.
2. Enable root MFA; do not create root access keys.
3. Create separate least-privilege IAM roles for Terraform and deployment.
4. Start with one small EC2 instance and one Elastic IP in `eu-central-1`.
5. Use encrypted EBS and a restrictive security group: HTTPS only when needed, no public SSH (prefer SSM), and exactly one configured UDP test port in the first POC.
6. Provision only through Terraform and review every `terraform plan` before apply.

An Application Load Balancer does not proxy UDP. In the first POC, UDP terminates directly on the probe’s Elastic IP. API and probe can share an instance initially but remain separate services and trust boundaries.

## Security principles

A public UDP listener can become an abuse primitive. These controls are mandatory from the first implementation:

- No response without a valid, unexpired, single-use session.
- Per-test HMAC authentication; bind the session to the first validated endpoint source address and port.
- Hard caps on test duration, payload size, packet rate, concurrent tests and tests created per source IP.
- Never echo arbitrary inbound data and never accept a destination address from the client.
- HTTPS-only control plane; no cloud credentials or long-lived secrets in the agent.
- Log only session and aggregate test events. Do not log payloads. Retain no customer identity and minimise source-IP handling.
- Run containers unprivileged, scan dependencies and images in CI, and patch regularly.

See [docs/threat-model.md](docs/threat-model.md) for the security design baseline.

## Project status

**Milestone 1 — Local protocol foundation: in progress.** The Rust workspace, authenticated packet codec and an actual loopback agent↔probe integration test are implemented locally. The test proves one valid full-duplex exchange and that an unknown session receives no UDP response. Security/error-path coverage, an expiring source-bound session registry and a user-facing binary workflow are still outstanding. AWS remains intentionally untouched.

The authoritative delivery sequence is in [ROADMAP.md](ROADMAP.md).

## Repository layout

```text
.
├── crates/
│   ├── agent/                 # Endpoint CLI binary
│   ├── api/                   # HTTPS session/result API
│   ├── probe/                 # UDP probe daemon
│   └── protocol/              # Framed packet codec + HMAC
├── docs/
│   ├── architecture.md
│   ├── metrics.md
│   ├── report-contract.md
│   └── threat-model.md
├── infra/terraform/
├── .github/workflows/
├── AGENTS.md
├── Cargo.toml                  # Rust workspace manifest
├── ROADMAP.md
└── README.md
```

## Technology decisions

| Area | Decision | Rationale |
|---|---|---|
| Endpoint/probe/API | Rust | Native cross-platform binaries, predictable UDP handling, memory safety and static release artefacts. |
| Async runtime | Tokio | Mature async I/O and UDP primitives for agent, probe and API. |
| HTTP/API | Axum | Small Tokio-native HTTP layer with typed request/response handling. |
| Data store | SQLite locally; PostgreSQL only when history needs exceed local scope | Avoids premature infrastructure. |
| Compute | EC2 + Elastic IP | Direct UDP endpoint and explicit networking controls. |
| Infrastructure | Terraform | Reproducible, reviewable cloud state. |
| Packaging | Docker | Repeatable API/probe deployment; agent remains native. |
| CI | GitHub Actions | `fmt`, Clippy, tests, dependency audit and Terraform validation. |

## Local developer workflow — target state

The following commands become valid as implementation is added:

```bash
# Run unit and integration tests
cargo test --workspace

# Run static checks
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo audit

# Build binaries
cargo build --workspace --release

# Start local API and probe after compose configuration is added
docker compose up --build

# Execute a local test after the agent exists
./agent --api-url http://127.0.0.1:8080 --region local --duration 30s --json-out result.json

# Validate infrastructure without applying it
cd infra/terraform
terraform fmt -check
terraform validate
terraform plan
```

## Roadmap and contribution model

Read [ROADMAP.md](ROADMAP.md) before creating implementation work. The rules for human and AI contributors, including Vonage Codex usage, security boundaries and mandatory test gates, are in [AGENTS.md](AGENTS.md).

## Contributing and security

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening an issue or pull request. Review [SECURITY.md](SECURITY.md) for private vulnerability reporting and the project’s safe-testing boundary.

## License

Copyright 2026 Marcin Wiatr. Licensed under the [Apache License 2.0](LICENSE). See [NOTICE](NOTICE) for attribution information.

## Disclaimer

This project produces diagnostic measurements between its endpoint agent and selected probe only. It is not a certification tool and cannot replace packet capture analysis, application logs, real-time media statistics, or investigation of the actual production call path.

# Network Readiness Probe

> A personal, self-hosted engineering proof of concept for measuring **bidirectional UDP network quality** between an endpoint and a project-controlled regional probe.

[![Status: design](https://img.shields.io/badge/status-design-blue)](#project-status)
[![Language: Rust](https://img.shields.io/badge/language-Rust-DEA584)](#technology-decisions)
[![Infrastructure: Terraform](https://img.shields.io/badge/infrastructure-Terraform-7B42BC)](#aws-poc-boundary)

## Important boundary

This repository is an independent personal project. It uses only infrastructure controlled by this project and must not use customer data, internal endpoints, credentials, packet captures, or call content.

A result is a point-in-time measurement between the runner and the selected controlled probe. It does not certify a network, guarantee call quality, or replace diagnostics of an actual production media path.

## Problem statement

HTTP/TCP speed tests can show bandwidth and a basic latency result, but they do not provide directional, time-sensitive evidence about real-time media conditions. This project is intended to measure:

- UDP reachability on the configured high-numbered UDP listener;
- uplink and downlink loss independently;
- loss bursts, duplicates, and out-of-order packets;
- jitter percentiles rather than an undefined average;
- RTT under a defined traffic profile; and
- sustained received payload throughput independently in each direction.

The test is bounded, authenticated, and full duplex. It returns raw evidence with deliberately limited interpretation.

## Goals

- Build a cross-platform Rust endpoint agent for Windows, Linux, and macOS.
- Operate a controlled regional UDP probe, beginning in Frankfurt (`fra` / `eu-central-1`).
- Measure directional loss, jitter p50/p95, RTT, and achieved payload throughput.
- Produce a transparent report with environment metadata, selected profile, directional media metrics, and controlled-endpoint reachability.
- Make execution safe with short-lived session credentials, strict rate limits, fixed duration, and no UDP reflection behavior.
- Define permanent cloud infrastructure in Terraform and keep the first release focused on diagnostic integrity.

## Non-goals for the MVP

- Public production service or customer-support workflow.
- Network or service certification, vendor-specific claims, or production-path testing.
- SIP registration, credentials, port scanning, or arbitrary destination probing.
- A generic UDP echo service.
- MOS before codec, packetization, impairment, and delay assumptions are documented and validated.
- A desktop browser shell; the agent remains a small native Rust binary.

## Architecture

```text
+-------------------------------------+
| Endpoint agent (Rust CLI)           |
|-------------------------------------|
| - requests a one-time session       |
| - sends authenticated UDP frames    |
| - observes downlink frames          |
| - emits a structured result         |
+----------------+--------------------+
                 | HTTPS: session + result
                 | UDP: bounded packet train
                 v
+-------------------------------------+       +------------------------------------+
| API / session service               |<----->| Regional UDP probe                 |
|-------------------------------------|       |------------------------------------|
| - issues test ID and HMAC material  |       | - active-session registry          |
| - expiry and rate limiting          |       | - controlled reverse UDP stream    |
| - result schema validation          |       | - authoritative UL observation     |
+----------------+--------------------+       +----------------+-------------------+
                 |                                             |
                 +---------------- AWS eu-central-1 -----------+
```

The probe is authoritative for packets received from the agent (**uplink**); the agent is authoritative for packets received from the probe (**downlink**). Neither direction is inferred from the other observer.

## Canonical UDP frame (v2)

The implemented packet codec is the canonical wire contract. All multi-byte fields are big-endian; the timestamp is monotonic-relative and is never UTC.

| Offset | Size | Field |
|---:|---:|---|
| 0–3 | 4 | Magic `[0x4E, 0x52, 0x50, 0x02]` (`NRP\\x02`) |
| 4 | 1 | Protocol version `0x02` |
| 5–20 | 16 | Opaque test ID |
| 21 | 1 | Direction: `0x01` uplink (agent → probe), `0x02` downlink (probe → agent) |
| 22–25 | 4 | `u32` sequence number |
| 26–33 | 8 | `i64` monotonic send timestamp in nanoseconds |
| 34–35 | 2 | `u16` payload length, 0–1200 bytes |
| 36… | N | Random payload |
| 36+N… | 32 | HMAC-SHA256 over every preceding frame byte |

The fixed header is 36 bytes and the HMAC tag is 32 bytes. Valid frames are **68–1268 bytes** inclusive. Decoders reject short frames, invalid magic/version/direction, oversized declared payloads, length mismatches, and invalid HMACs before accepting a packet.

See [the architecture contract](docs/architecture.md#canonical-udp-frame-format-v2) for lifecycle and validation order, and [the metric specification](docs/metrics.md) for result semantics.

## Measurement model

1. The agent calls `POST /v1/tests` over HTTPS.
2. The API rate-limits the request and returns a CSPRNG test ID, expiry, exact probe target, and one-time HMAC material.
3. The agent sends authenticated uplink frames only to that returned target.
4. After validation, the probe binds the session to the first verified source `IP:port`, records uplink observations, and sends only the configured downlink stream to that bound endpoint.
5. The agent records downlink observations and RTT, then submits exactly one versioned result to `POST /v1/results`.
6. The API validates session state, expiry, schema, and ranges; packet payloads are never stored.

A failed validated handshake is `UDP_UNREACHABLE_OR_BLOCKED`, not 100% loss and not a fabricated quality measurement.

## UDP listener and AWS POC boundary

The first POC is private-development and limited proof-of-concept use only. It starts with one controlled listener in Frankfurt and a single documented UDP port: **`10000` or `16384`**. A listener in this high-numbered media-port range is necessary because a successful test on a well-known port does not establish reachability for that path. The security group permits exactly the selected UDP port; UDP terminates directly on the probe Elastic IP rather than through an ALB.

Before a public resource is created:

1. Configure low AWS Budget thresholds and billing alerts.
2. Enable root MFA and do not create root access keys.
3. Use separate least-privilege Terraform and deployment roles.
4. Use encrypted EBS, no public SSH (use SSM), and restrictive ingress.
5. Provision only through Terraform after a reviewed `terraform plan`.

## Security principles

- No UDP response without a valid, unexpired, single-use session.
- Per-test HMAC authentication and binding to the first validated endpoint source address and port.
- Hard ceilings for duration, payload, packet rate, concurrent tests, and session creation.
- No arbitrary echo, no client-provided UDP destination, and no response to unknown traffic.
- HTTPS-only control plane; no cloud credentials or long-lived probe secrets in the agent.
- No packet payload logging; retain only the minimal aggregate result and controlled diagnostics.

See [docs/threat-model.md](docs/threat-model.md) for abuse cases, controls, and release gates.

## Project status

**Milestone 1 — Local protocol foundation: in progress.** The Rust workspace, canonical authenticated packet codec, loopback agent↔probe integration test, and an expiring source-bound registry primitive exist locally. Current work still needs complete negative-path codec coverage, integration of registry state into a bounded multi-packet receive loop, terminal completion handling, and a user-facing binary workflow. AWS remains intentionally untouched.

The authoritative delivery sequence is [ROADMAP.md](ROADMAP.md).

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
| Endpoint/probe/API | Rust | Native binaries, predictable UDP handling, and memory safety. |
| Async runtime | Tokio | UDP and HTTP primitives for the Rust services. |
| HTTP/API | Axum | Small Tokio-native typed HTTP layer. |
| Data store | SQLite locally; PostgreSQL later if scope requires | Avoids premature infrastructure. |
| Compute | EC2 + Elastic IP | Direct UDP endpoint and explicit networking controls. |
| Infrastructure | Terraform | Reproducible, reviewable cloud state. |
| Packaging | Docker | Repeatable API/probe deployment; agent remains native. |
| CI | GitHub Actions | Formatting, linting, tests, dependency audit, and infrastructure validation. |

## Local developer workflow — target state

```bash
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo audit
cargo build --workspace --release

# After local compose configuration exists
docker compose up --build

# Validate infrastructure without applying it
cd infra/terraform
terraform fmt -check
terraform validate
terraform plan
```

## Contributing and security

Read [ROADMAP.md](ROADMAP.md) before implementation work. Contributor workflow and mandatory checks are in [CONTRIBUTING.md](CONTRIBUTING.md); safe-testing and vulnerability reporting are in [SECURITY.md](SECURITY.md).

## License

Copyright 2026 Marcin Wiatr. Licensed under the [Apache License 2.0](LICENSE). See [NOTICE](NOTICE) for attribution information.

## Disclaimer

This project produces diagnostics only for the path between its agent and selected controlled probe. It cannot replace application telemetry, real media statistics, packet captures, or investigation of an actual production path.

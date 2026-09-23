# Architecture

## Context

Network Readiness Probe measures network behavior between an endpoint agent and a probe controlled by this project. It is not an implementation of a third-party platform test and must not target external production services.

## Components

### Endpoint agent

A signed/static Rust CLI released for Windows, Linux and macOS. Responsibilities:

- requests an HTTPS test session;
- performs authenticated UDP handshake;
- schedules bounded outgoing frames;
- receives reverse frames and calculates downlink metrics;
- submits result and writes a local JSON copy;
- exits safely on cancellation, expiry or loss of connectivity.

The agent never contains AWS credentials, long-lived probe secrets or customer identifiers.

### API/session service

A small HTTPS service. Responsibilities:

- creates random one-time session records;
- returns probe target, expiry and short-lived per-session HMAC material;
- applies session-creation rate limits;
- receives and validates final result documents;
- provides health and minimal administration endpoints.

### UDP probe

A Rust daemon using Tokio with a public UDP listener. Responsibilities:

- validates frame format, session ID, HMAC and expiry;
- binds a valid session to the first verified source address and port;
- emits only the bounded reverse stream defined by the active session;
- observes endpoint-originated sequence numbers for uplink metrics;
- produces a server-side summary.

It is not an echo server. It never takes a target address from the client.

### Result store

SQLite is sufficient locally. The stored record contains test metadata, aggregate numeric metrics, warnings, agent/probe versions and retention timestamps. Packet payloads are never stored.

### Milestone 1 local execution contract

Before an API or any public listener exists, local tests use loopback only. A session is constructed explicitly in memory with an opaque 16-byte test ID, a per-session HMAC key and an expiry. The intended probe contract is to bind a session to the first source `IP:port` from a valid authenticated uplink frame and emit downlink frames only to that bound address, only while valid, without mirroring received payload bytes.

The current implementation expresses this contract as a local Rust integration test. The probe rejects a socket not bound to a loopback address, receives exactly one **uplink** datagram for one explicit session, and returns a fixed 172-byte **downlink** frame only after successful HMAC and test-ID validation. The tests also prove an unknown test ID receives no UDP response. The expiring registry, durable source binding and user-facing local binary commands remain Milestone 1 follow-up work.

## Trust boundaries

```text
[Untrusted endpoint/network]
          |
          | HTTPS session request / result upload
          v
[API: validates creation, schema, expiry] ---- private control ---- [Probe]
          ^                                                    |
          |                                                    | authenticated bounded UDP only
          +----------------------------------------------------+
```

- Endpoint traffic is untrusted until session and HMAC validation complete.
- API-to-probe control is private/authenticated and must not be reachable from the internet.
- Results are untrusted input until schema and session association are validated.
- AWS credentials are deployment/runtime secrets and never cross to the endpoint.

## Interface contracts

### `POST /v1/tests`

Request (MVP): region preference only; no customer identity.

Response:

```json
{
  "test_id": "opaque-random-id",
  "expires_at": "2026-09-23T19:00:00Z",
  "probe": {"region": "eu-central-1", "host": "example.invalid", "udp_port": 4433},
  "hmac_key": "base64url-one-time-secret",
  "max_duration_seconds": 60,
  "max_packet_rate": 100
}
```

Actual fields and encodings must be defined by a versioned JSON schema before implementation. Avoid using a GET endpoint so browser/proxy logs do not inadvertently expose session material.

### `POST /v1/results`

Accepts exactly one schema-valid document associated with a valid session. The API must validate expiry, agent version compatibility, value ranges and duplicate submission semantics.

## Deployment topology: first POC

One EC2 instance with an Elastic IP in `eu-central-1` may run the API and UDP probe as separate containers. An HTTPS reverse proxy terminates API TLS. The UDP socket binds directly to the EC2 instance; an ALB is not part of the UDP path.

This topology is temporary. API and probe become separate instances/services when scale, blast radius or deployment cadence warrants it.

## Decisions deferred

- Persistent multi-tenant database and authentication.
- Browser UI and result sharing.
- Additional regions.
- Load/ramp profile.
- SIP ALG marker module.
- MOS/E-model output.

Any decision that adds public protocol surface, persistent identity, credential handling or third-party integrations needs a documented ADR and threat-model update.

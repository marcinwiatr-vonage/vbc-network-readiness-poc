# Architecture

## Context and boundary

Network Readiness Probe is a personal, self-hosted engineering POC that measures the path between an endpoint agent and a project-controlled regional probe. It supplies transparent directional UDP evidence; it is not a certification and does not target production voice infrastructure.

The first POC runs only in `fra` / `eu-central-1`. Any future probe is independently deployed, monitored, and controlled by this project.

## UDP listener boundary

Firewalls and middleboxes can treat high-numbered UDP differently from well-known ports. A successful result on another port does not prove reachability to the configured media-path listener. The public probe therefore exposes exactly one documented UDP listener in the high-numbered range: **`10000` or `16384`**. The security group permits exactly the selected port. Optional TCP reachability checks are distinct measurements and must not be represented as UDP evidence.

## Components

### Endpoint agent

A Rust CLI for Windows, Linux, and macOS. It requests a session, sends bounded authenticated UDP frames only to the API-returned target, observes downlink frames, calculates client-authoritative downlink metrics, and emits structured JSON.

It must never store cloud credentials, long-lived secrets, or customer identity; send UDP to an arbitrary destination; infer one-way latency from RTT; or claim certification.

### API/session service

A Rust HTTP service that creates one-time sessions, selects a probe, returns short-lived per-session HMAC material, validates one result, and exposes health endpoints.

It must never return HMAC material twice; accept an expired/replayed result; log packet payloads; or retain raw source IP beyond the documented minimum retention policy.

### UDP probe

A Rust UDP service that validates framing, HMAC, test ID, direction, expiry, and source binding before emitting the bounded downlink stream. It is authoritative for uplink receipt observations.

It must never emit traffic without a valid authenticated session; use a client-provided destination; respond to unauthenticated/unknown traffic; or act as an echo service.

### Result store

SQLite is sufficient locally; a persistent store beyond that is deferred until scope warrants it. It stores aggregate values, schema versions, safe diagnostics, and retention timestamps.

It must never persist packet payloads, raw source IP after its retention window, or expired test IDs.

## Session lifecycle

1. The agent requests a region-selected session over HTTPS.
2. The API rate-limits creation, creates a CSPRNG test ID and one-time HMAC key, selects the probe, and records expiry/rate/duration ceilings.
3. The agent receives the explicit probe host, UDP port, expiry, and key; it sends authenticated uplink frames only to that target.
4. The probe validates framing before session lookup, then validates HMAC, session, expiry, and source binding; the first verified source `IP:port` becomes the only permitted response destination.
5. The probe records uplink observations and sends only the configured downlink stream to that bound source.
6. The agent records downlink observations and RTT, then creates a versioned JSON result.
7. The API validates session state, report schema, and ranges before accepting one aggregate result. Raw payloads are never stored.

## Canonical UDP frame format (v2)

The prior experimental v1 layout (`NRP1`, 40-byte header) is retired and is deliberately unsupported. No public probe or released agent used it; mixed-format operation is prohibited. All new agents and probes must use this v2 format, and decoders reject the retired magic before session lookup.

```text
Offset  Size  Type      Field
0       4     u8[4]     magic: [0x4E, 0x52, 0x50, 0x02] ("NRP\\x02")
4       1     u8        protocol_version: 0x02
5       16    u8[16]    test_id (opaque 128-bit session ID)
21      1     u8        direction: 0x01=UL (agent→probe), 0x02=DL (probe→agent)
22      4     u32 BE    sequence_number (monotonic per direction; starts at 1)
26      8     i64 BE    monotonic_send_timestamp_ns
34      2     u16 BE    payload_length (0–1200 inclusive)
36      N     u8[N]     random_payload
36+N    32    u8[32]    HMAC-SHA256 over bytes [0, 36+N)
```

The header is 36 bytes; the authentication tag is 32 bytes. A zero-payload frame is 68 bytes, and a 1200-byte-payload frame is 1268 bytes. Valid frames are therefore 68–1268 bytes inclusive.

### Decoder validation order

1. Reject datagrams shorter than 68 bytes.
2. Reject invalid magic and unsupported version before session lookup.
3. Reject unknown direction.
4. Parse the fixed-size fields and reject payload lengths above 1200 before variable payload allocation.
5. Require the datagram length to equal `36 + payload_length + 32`.
6. Verify HMAC-SHA256 over bytes through the payload before accepting the frame.
7. Only then validate session, expiry, expected direction, and source binding.

Sequence numbers are independent `u32` streams in each direction. Wraparound behavior must be deliberately specified and tested before release; it is not inferred by the wire decoder.

## Local session lifecycle, bounds, and replay policy

Milestone 1 registry entries use an injected **monotonic** deadline; an entry is expired when `now >= deadline`, regardless of wall-clock changes. Conversion from the local bootstrap API's wall-clock expiry uses checked monotonic arithmetic and fails closed when the duration cannot be represented. “Single-use” means one atomic transition from unbound to the first verified source `IP:port`, followed by one bounded active lifetime—not one accepted packet.

During that lifetime, the registry authorizes only authenticated uplink frames from the bound source with a non-zero, strictly increasing sequence number. Gaps are permitted so loss can be measured; duplicate, lower, zero, and out-of-order frames are silently rejected. Each successful authorization returns a short immutable permit whose destination is exclusively the observed bound source. Expiry, source binding, replay admission, and the packet ceiling are checked and mutated atomically before that permit is returned.

The implemented local receive loop binds only to an IPv4 or IPv6 loopback address and admits at most **32 authenticated uplink packets**. Its effective monotonic deadline is the earlier of the session deadline and **two seconds after loop start**. Each admitted uplink produces one fixed 172-byte-payload downlink frame, numbered from 1 in admission order and sent only to the permit destination. Reaching the packet ceiling or effective deadline terminates the loop; malformed, invalid-HMAC, unknown-session, expired, replayed/out-of-order, wrong-direction, or source-rebound traffic produces no response and does not consume the admitted-packet budget. Terminal completion state beyond loop termination remains deferred Milestone 1 work.

### Local CLI bootstrap contract

The Milestone 1 binaries use an operator-created, local-only session file instead of the future HTTPS control plane. The file is UTF-8 text, is limited to 1 KiB, and has exactly these five newline-delimited records in this order:

```text
NRP-LOCAL-SESSION-V1
test_id_hex=<32 lowercase hexadecimal characters>
hmac_key_hex=<64 lowercase hexadecimal characters>
expires_at_unix_seconds=<unsigned decimal UTC epoch seconds>
probe_address=127.0.0.1:<10000-or-16384>
```

Unknown, duplicated, reordered, missing, non-canonical, or trailing records are rejected. The target must be the IPv4 loopback address and exactly UDP `10000` or `16384`; hostnames, wildcard addresses, additional ports, and client-selected remote destinations are not accepted. The file contains one-time HMAC material, so it must be created outside the repository, permission-restricted by the operator, and deleted after the run. No example with a usable credential is committed.

`probe --session-file <path>` binds only to the address in that validated file and still applies the earlier of file expiry or the two-second monotonic runtime ceiling. `agent --session-file <path> [--response-timeout-ms <1..=2000>]` sends the fixed 32-packet local profile only to the validated address. Invalid arguments, unreadable or invalid session files, expired sessions, bind failures, and internal protocol/I/O failures are explicit non-zero errors; only the expected probe receive timeout at its bounded deadline produces a normal `DEADLINE_REACHED` summary.

Each binary writes one compact JSON object to standard output for an executed run. This is a versioned **local run summary**, not the Milestone 2 report schema and not a persisted result. The agent reports `COMPLETED` only after all 32 authenticated responses, or `UDP_UNREACHABLE_OR_BLOCKED` with unavailable counts represented as `null` when no validated response arrives before the bounded timeout. The probe reports whether its packet ceiling was completed or its deadline was reached, plus admitted and sent counts. One-time HMAC material is never emitted.


```text
[Untrusted endpoint/network]
   | HTTPS: session request/result
   v
[Regional API/session service] -- private control --> [Probe: fra]
      |                                         authenticated UDP only
      |------------------------------------------------------^

Future regions: [Probe: lon] [Probe: va] [Probe: or] [Probe: sg] [Probe: sy]
Each region is an independent trust boundary. The POC has no cross-region probe control plane.
```

Endpoint traffic is untrusted until frame/HMAC/session validation completes. Results are untrusted until schema/session validation completes. Deployment credentials never reach agents.

## Interface contracts

`POST /v1/tests` returns a new one-time session. The probe target is a project-controlled high-numbered UDP listener:

```json
{
  "test_id": "opaque-random-id",
  "expires_at": "2026-09-23T19:00:00Z",
  "probe": {"region": "fra", "host": "probe-fra.<your-domain>.invalid", "udp_port": 10000},
  "hmac_key": "base64url-one-time-secret",
  "max_duration_seconds": 60,
  "max_packet_rate": 100
}
```

`POST /v1/results` accepts exactly one schema-valid, unexpired result associated with the session. Report fields and unavailable-value semantics are defined in [report-contract.md](report-contract.md).

## Regional probes

| Label | AWS Region | Probe hostname pattern | Listener | Live from |
|---|---|---|---|---|
| fra | eu-central-1 | `probe-fra.<your-domain>.invalid` | `10000` or `16384` | Milestone 5 |
| lon | eu-west-2 | `probe-lon.<your-domain>.invalid` | `10000` or `16384` | Milestone 8 |
| va | us-east-1 | `probe-va.<your-domain>.invalid` | `10000` or `16384` | Milestone 8 |
| or | us-west-2 | `probe-or.<your-domain>.invalid` | `10000` or `16384` | Milestone 8 |
| sg | ap-southeast-1 | `probe-sg.<your-domain>.invalid` | `10000` or `16384` | Milestone 8 |
| sy | ap-southeast-2 | `probe-sy.<your-domain>.invalid` | `10000` or `16384` | Milestone 8 |

Only one listener is selected per deployed probe. `TBD` values such as a public IP are not a target and must not be placed in sessions until provisioned by Terraform.

## Deployment topology

The first deployment runs the Rust API and probe as separate containers on one EC2 instance with an Elastic IP. The UDP socket binds directly to the instance; an ALB is not on the UDP path. All permanent infrastructure is Terraform-managed.

## Decisions deferred

- Persistent multi-tenant authentication and browser UI.
- Load/ramp profile and estimated call capacity.
- Additional regions after stable Frankfurt operation.
- Controlled SIP marker test.
- MOS/E-model.

Any new public protocol surface, identity collection, persistent credential handling, or additional service integration requires an ADR and a threat-model update.

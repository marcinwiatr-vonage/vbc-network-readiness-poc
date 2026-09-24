# Test Report Contract

This document defines the report shape, field ownership, persistence limits, and unavailable-value semantics for Network Readiness Probe. It is a requirements contract for measurements against this project's controlled endpoints.

The Milestone 1 local agent/probe CLI JSON is a bootstrap run summary only. It is not this report contract, is not persisted, and deliberately omits directional quality metrics until the Milestone 2 calculators and versioned result schema exist.

## Report principles

`protocol_version` is the wire-frame version. The current canonical frame is v2; the earlier experimental v1 layout is retired and must be rejected rather than silently decoded.

- Show the selected probe ID, region, and exact controlled UDP port used by the test.
- Keep upstream and downstream measurements separate and identify their authoritative observer.
- Display raw values, units, and schema/threshold versions next to any condition label.
- Treat identity and network metadata as sensitive: show it to the runner only when needed and do not persist it by default.
- Test reachability only to explicitly controlled endpoints; a report is not a port scan.
- Never mark a port reachable without the protocol-appropriate observed result.
- Use `null` for an attempted measurement without a valid value and `not_tested` for a measurement that was intentionally not run.

## Header and environment data

| Field | MVP | Collection method | Persistence rule |
|---|---:|---|---|
| `started_at` / `ended_at` | Yes | Agent UTC timestamps. | Store timestamps. |
| `connection_type` | Best effort | Agent classifies active interface as Ethernet/Wi-Fi/unknown. | Store class only. |
| `asn` / `provider` | Later | Server-side mapping of probe-observed public source address. | Store only after retention design; never raw lookup input. |
| `public_ip_display` | Runner-visible, optional | Probe-observed source address, never client supplied. | Do not persist raw IP by default; history uses a masked value if enabled. |
| `country_code` | Later | Server-side mapping of probe-observed address. | Store only with a retention policy. |
| `http_proxy_status` | Best effort | `configured`, `not_configured`, or `unknown`; it does not claim generic proxy/VPN detection. | Store status only. |
| `probe_id` / `probe_region` / `udp_port` | Yes | API-selected session target actually used. | Store values. |
| `agent_version` / `protocol_version` / `report_schema_version` | Yes | Agent/session metadata. | Store values. |
| `runner_label` | Optional | Free-text local label. | Do not store by default. |

## Measurement and profile fields

| Field | MVP | Source of truth / definition |
|---|---:|---|
| `profile` | Yes | Selected named packet profile and its documented parameters. |
| `upstream.packets_*`, loss, jitter, throughput | Yes | Probe-observed authenticated UL stream. |
| `downstream.packets_*`, loss, jitter, throughput | Yes | Agent-observed authenticated DL stream. |
| `rtt_ms` | Yes | Agent monotonic elapsed time for matching authenticated request/response pairs. |
| `estimated_supported_calls` | Later | Only after the configured profile and degradation rule are documented and tested. |
| `mos` | Later | Only after documented codec, packetization, impairment, delay, formula, and validation. |
| `sip_alg` | Later, optional | Isolated controlled marker test only; never credentials, arbitrary destinations, or production targets. |

Definitions and units are normative in [metrics.md](metrics.md). `upstream` and `downstream` must never be collapsed into a single loss, jitter, or throughput value.

## Controlled reachability

The MVP report contains one UDP reachability observation for the session target:

```text
UDP 10000 or UDP 16384    Bidirectional / No validated response / Not tested
```

Only the provisioned session port is shown. `Bidirectional` requires a validated authenticated exchange; `No validated response` maps to `UDP_UNREACHABLE_OR_BLOCKED`; `Not tested` means the test deliberately did not run. Optional HTTPS/API connectivity may be reported separately as control-plane reachability, not UDP reachability.

## Status labels and thresholds

A UI can render configurable labels such as `UDP unreachable`, `uplink impairment`, `downlink impairment`, `variable latency`, and `insufficient configured-profile capacity`. These labels are presentation configuration, not protocol truth.

Any configured threshold set must carry a `threshold_version`, list its units and comparison boundaries, use p95 jitter and direction-specific loss, and show the underlying raw values. An unavailable value cannot produce a green result. If MOS is absent, render `not_tested` or `null` as applicable; do not infer it from a colour.

## Required unavailable-value semantics

| Situation | `status` | Metric values | Reachability / optional fields |
|---|---|---|---|
| Valid bidirectional test | `COMPLETED` | Measured values; `null` only where an individual calculation lacks eligible samples. | `bidirectional`; optional unrun field is `not_tested`. |
| No validated UDP response | `UDP_UNREACHABLE_OR_BLOCKED` | All loss, jitter, RTT, and throughput values are `null`. | `no_validated_response`; unrun fields are `not_tested`. |
| Session/HMAC/expiry/replay/source rejection | `SESSION_REJECTED` | All measurement values are `null`. | `not_tested` unless a separate control-plane observation occurred. |
| User cancellation before completed window | `CANCELLED` | `null` unless a completed measurement value is explicitly safe and documented. | `not_tested` where the check was skipped. |
| Internal failure | `INTERNAL_ERROR` | `null` unless independently completed and explicitly marked safe. | `not_tested` where no observation exists. |

## Example result structure

The example is illustrative only; final field names are fixed by the versioned result schema.

```json
{
  "report_schema_version": 1,
  "protocol_version": 2,
  "test_id": "opaque-id",
  "status": "COMPLETED",
  "started_at": "2026-09-23T18:35:00Z",
  "ended_at": "2026-09-23T18:36:00Z",
  "environment": {
    "connection_type": "ethernet",
    "public_ip_display": "203.0.113.xxx",
    "probe_id": "fra-1",
    "probe_region": "fra",
    "udp_port": 10000,
    "agent_version": "0.1.0"
  },
  "profile": {"name": "udp-baseline-v1", "payload_bytes": 172, "target_packet_rate": 100},
  "upstream": {
    "observer": "probe",
    "packets_sent": 6000,
    "packets_received": 5998,
    "loss_pct": 0.033,
    "jitter_ms_p50": 0.5,
    "jitter_ms_p95": 1.2,
    "throughput_kbps": 137.6
  },
  "downstream": {
    "observer": "agent",
    "packets_sent": 6000,
    "packets_received": 6000,
    "loss_pct": 0.0,
    "jitter_ms_p50": 0.5,
    "jitter_ms_p95": 1.3,
    "throughput_kbps": 137.7
  },
  "rtt_ms": {"min": 11.0, "mean": 12.0, "p95": 16.0},
  "reachability": {"protocol": "udp", "port": 10000, "status": "bidirectional"},
  "sip_alg": {"status": "not_tested"},
  "mos": {"upstream": null, "downstream": null}
}
```

Packet payloads, one-time HMAC material, and raw source IP are never report fields or persistent result fields.

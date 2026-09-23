# Test Report Contract

This document defines the target report shape inspired by the reference VoIP assessment. It is a requirements contract, not an assertion that the project tests any third-party service.

## Report principles

- Show the target region/probe actually used by the test.
- Keep **upstream** and **downstream** results separate.
- Display raw measurement values next to a status label.
- Treat identity/network metadata as sensitive: show it to the test runner, but do not persist it by default.
- Test firewall reachability only to this project’s explicitly controlled probe endpoints and ports.
- Never mark a port `Open` unless a protocol-appropriate connection/response was observed.

## Header and environment data

| Field | MVP | Collection method | Persistence rule |
|---|---:|---|---|
| Date/time | Yes | Agent UTC start/end timestamp. | Store timestamp. |
| Connection type | Yes, best effort | Agent detects active interface and classifies Ethernet/Wi-Fi/unknown. OS-specific implementation; do not overclaim accuracy. | Store class only. |
| Provider / ASN | Yes | Probe observes public source IP; server-side GeoIP/ASN database maps ASN and organisation. | Store ASN/org; hash IP by default. |
| Public IP address | Yes, runner-visible | Probe-observed source address, not a browser-supplied value. | Do not persist raw IP by default; show masked value in history. |
| Country | Yes | Server-side GeoIP mapping of probe-observed public IP. | Store country code. |
| Proxy status | Yes, qualified | Report `HTTP proxy configured`, `HTTP proxy not configured`, or `unknown`; never claim generic VPN/proxy detection. | Store status only. |
| Selected probe / server location | Yes | API-selected region and probe ID. | Store region/probe ID. |
| Company / label | Optional | Free-text local label supplied by runner. | Do not store by default; no customer names in POC. |
| Test history | Later | Local/session result history with explicit retention. | Requires retention/deletion policy. |

## Capacity results

| Field | MVP | Definition |
|---|---:|---|
| Target calls / traffic profile | Yes | Selected named profile, e.g. `g711-100kbps-v1`; show assumed aggregate rate and duration. Do not label this a vendor/platform capacity. |
| Downstream capacity | Yes | Probe-to-agent received payload throughput during the defined full-duplex load profile. |
| Upstream capacity | Yes | Agent-to-probe received payload throughput during the same profile. |
| Estimated supported calls | Later | Derived only after the configured traffic profile and degradation rule are documented and tested. |

## VoIP/media results

| Field | MVP | Definition |
|---|---:|---|
| Upstream jitter | Yes | Probe-observed UL jitter; show p50 and p95, with p95 used for status. |
| Downstream jitter | Yes | Agent-observed DL jitter; show p50 and p95, with p95 used for status. |
| Upstream loss | Yes | Missing authenticated endpoint sequence numbers observed by probe. |
| Downstream loss | Yes | Missing authenticated probe sequence numbers observed by agent. |
| Latency | Yes | RTT min/mean/p95. Label as RTT, never one-way latency. |
| Upstream/downstream MOS | Later | Requires documented codec, packetization, delay/loss impairment model and validation. |
| SIP ALG | Later, optional | Controlled marker mutation test against this project’s isolated test endpoint only. Never uses credentials or targets a production PBX/SBC. |

## Firewall reachability results

The report can display a configurable test-port matrix, for example:

```text
TCP <controlled HTTPS/API port>    Connected / Blocked / Not tested
UDP <controlled probe port>        Bidirectional / No validated response / Not tested
TCP <controlled TLS test port>     Connected / Blocked / Not tested
```

The reference display includes TCP 10006, TCP 10002, UDP 5060, TCP 5061 and TCP 636. Those exact ports must **not** be copied as a default public scan. If we later use them, they must terminate only on project-controlled endpoints, be explicitly enabled in Terraform, and have a documented test purpose. UDP 5060/SIP-related testing remains blocked until the optional SIP ALG milestone and a dedicated threat-model review.

## Status thresholds

The UI can support these initial configurable thresholds, matching the reference display, but thresholds remain product configuration rather than protocol truth:

| Status | Jitter | RTT | Packet loss | MOS (when implemented) |
|---|---:|---:|---:|---:|
| Green | `< 5 ms` | `≤ 100 ms` | `< 0.1%` | `> 3.8` |
| Yellow | `5–<20 ms` | `>100–200 ms` | `0.1–1%` | `3.5–3.8` |
| Red | `≥ 20 ms` | `> 200 ms` | `> 1%` | `< 3.5` |

Status uses the worst applicable metric. For MVP, use p95 jitter and directional loss separately: do not hide one bad direction behind an overall green result. If MOS is absent, display `Not calculated` and do not infer it from the colour.

## Example result structure

```json
{
  "test_id": "opaque-id",
  "started_at": "2026-09-23T18:35:00Z",
  "environment": {
    "connection_type": "ethernet",
    "public_ip_display": "163.116.xxx.xxx",
    "asn": 55256,
    "provider": "Example Provider",
    "country_code": "PL",
    "http_proxy_status": "configured",
    "probe_region": "eu-central-1"
  },
  "profile": {"name": "g711-100kbps-v1", "target_calls": 2},
  "upstream": {"capacity_kbps": 9530, "loss_pct": 0.0, "jitter_ms_p50": 0.5, "jitter_ms_p95": 1.2},
  "downstream": {"capacity_kbps": 10000, "loss_pct": 0.0, "jitter_ms_p50": 0.5, "jitter_ms_p95": 1.3},
  "rtt_ms": {"min": 11, "mean": 12, "p95": 16},
  "firewall": [
    {"protocol": "udp", "port": 4433, "status": "bidirectional"}
  ],
  "sip_alg": {"status": "not_tested"},
  "mos": {"upstream": null, "downstream": null}
}
```

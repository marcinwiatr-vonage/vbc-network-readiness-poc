# Metrics Specification

This document defines the terms used in a Network Readiness Probe result. Implementations must not change units, direction semantics or labels without updating this document and associated tests.

## Time base

Use a monotonic clock for packet scheduling, test duration and inter-arrival calculations. Wall-clock UTC is permitted only for report timestamps and session expiry.

## Packet accounting

Each direction has an independent monotonically increasing sequence number.

- `packets_sent`: frames deliberately emitted within the measurement window.
- `packets_received`: valid authenticated frames received within that window.
- `duplicates`: valid frames with sequence number already observed.
- `out_of_order`: valid first-seen frames with a sequence lower than the largest first-seen sequence.
- `missing`: expected sequence numbers absent after a defined reordering grace period.

`loss_pct = 100 * missing / expected_packets`.

Do not count malformed, unauthenticated or expired-session datagrams as received frames.

## Directional semantics

| Direction | Sender | Receiver/observer | Report meaning |
|---|---|---|---|
| Uplink | Endpoint agent | Regional probe | Quality from endpoint/network towards probe. |
| Downlink | Regional probe | Endpoint agent | Quality from probe towards endpoint/network. |

The probe produces the authoritative uplink packet observation. The agent produces the authoritative downlink observation.

## Jitter

MVP jitter is calculated from per-packet receive inter-arrival deltas within one direction. The exact algorithm must be implemented once in a pure function with fixtures.

Report `jitter_ms_p50` and `jitter_ms_p95`; do not describe a percentile as an average. If an RTP-compatible RFC 3550 interarrival-jitter estimate is later used, add a separate field and name it precisely rather than silently replacing this definition.

## RTT

RTT is the elapsed monotonic time between an acknowledged timestamp request and its corresponding response. Report `rtt_ms_min`, `rtt_ms_mean` and `rtt_ms_p95`.

RTT is not one-way latency. The project does not estimate one-way latency without a valid clock-synchronization design.

## Throughput

`throughput_kbps = (received_payload_bytes * 8) / measurement_seconds / 1000`.

Report payload throughput separately from wire bitrate. Any later wire-rate estimate must include and document IP/UDP/frame overhead assumptions.

## Reachability outcomes

- `COMPLETED`: authenticated bidirectional exchange completed.
- `UDP_UNREACHABLE_OR_BLOCKED`: no validated probe handshake/response before timeout.
- `SESSION_REJECTED`: API or probe rejected session/HMAC/expiry.
- `CANCELLED`: agent was explicitly cancelled.
- `INTERNAL_ERROR`: implementation failure; include a safe diagnostic code but no secret material.

A reachability failure has no valid loss, jitter or throughput measurement. Values must be `null` rather than zero or a calculated percentage.

## Capacity and MOS

No generic capacity or MOS is calculated in MVP.

A future configured-profile capacity estimate must document payload size, packetization interval, transport overhead, total packet rate, duration, impairment threshold and the precise rule used to identify degradation. It must be labelled as an estimate for that named profile.

A future MOS calculation must document codec, packetization, delay and loss impairment assumptions, formula/version and validation. A MOS without those inputs is prohibited.

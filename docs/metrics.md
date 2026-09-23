# Metrics Specification

This document defines Network Readiness Probe measurement terms, units, source of truth, and unavailable-value semantics. Implementations must not change labels, direction, units, or algorithms without updating this document and associated fixtures.

## Time base and frame inputs

Use a monotonic clock for packet scheduling, test duration, RTT elapsed time, and receive inter-arrival calculations. UTC is allowed only for report timestamps and session expiry. The v1 frame carries an `i64` monotonic send timestamp at offsets 26–33; it is not a synchronized timestamp and cannot establish one-way latency.

Each valid v1 frame has an independent direction and `u32` sequence stream. Only frames that pass fixed-length validation, HMAC verification, session/expiry checks, expected-direction checks, and source-binding checks can contribute to a measurement.

## Packet accounting

Each direction has an independent monotonically increasing sequence number.

- `packets_sent`: frames deliberately emitted within the measurement window.
- `packets_received`: valid authenticated frames received within that window.
- `duplicates`: valid frames whose sequence was already observed.
- `out_of_order`: valid first-seen frames whose sequence is lower than the greatest first-seen sequence.
- `missing`: expected sequence numbers absent after the defined reordering grace period.

`loss_pct = 100 × missing / expected_packets`.

Do not count malformed, unauthenticated, wrong-direction, expired-session, or out-of-window datagrams as received frames. A zero expected-packet count yields `null`, not `0%`. Sequence-number wraparound semantics require a tested implementation decision before release.

## Directional source of truth

| Direction | Sender | Receiver / authoritative observer | Report meaning |
|---|---|---|---|
| Uplink (`UL`) | Endpoint agent | Regional probe | Quality from endpoint/network toward the controlled probe. |
| Downlink (`DL`) | Regional probe | Endpoint agent | Quality from controlled probe toward endpoint/network. |

The probe produces authoritative uplink receipt metrics. The agent produces authoritative downlink receipt metrics. Results must keep both observations distinct; neither direction may be calculated from the other observer.

## Jitter

MVP jitter is receive inter-arrival variation for valid, first-seen packets in a single direction. For consecutive eligible packets, calculate inter-arrival deltas from the receiver's monotonic clock; derive the documented variation sample in a pure function. Exclude duplicate, malformed, unauthenticated, out-of-window, and unavailable samples consistently.

Report `jitter_ms_p50` and `jitter_ms_p95`. A percentile is not an average. The report must identify the receiving observer and direction. If a later RTP-compatible RFC 3550 estimator is added, expose it as a separately named field rather than silently replacing this definition.

Use `null` when too few eligible samples exist for the defined percentile calculation.

## RTT

RTT is elapsed monotonic time between a timestamp request and its corresponding authenticated response. Report `rtt_ms_min`, `rtt_ms_mean`, and `rtt_ms_p95` with units of milliseconds.

RTT is not one-way latency. The project does not estimate one-way latency without a validated clock-synchronization design. If no corresponding response is observed, every RTT aggregate is `null`.

## Throughput

`throughput_kbps = (received_payload_bytes × 8) / measurement_seconds / 1000`.

Report received **payload** throughput separately for uplink and downlink. It excludes the 36-byte frame header, 32-byte HMAC tag, and IP/UDP overhead. Any future wire-rate estimate must be separately named and document all overhead assumptions. A non-positive measurement duration produces `null`.

## Reachability and status outcomes

| Status | Meaning | Directional metrics |
|---|---|---|
| `COMPLETED` | Authenticated bidirectional exchange completed. | Measured values or `null` where a specific calculation lacks samples. |
| `UDP_UNREACHABLE_OR_BLOCKED` | No validated probe handshake/response arrived before timeout. | Loss, jitter, RTT, and throughput are `null`; it is not 100% loss. |
| `SESSION_REJECTED` | API or probe rejected session, HMAC, expiry, replay, direction, or source binding. | All measurements are `null`. |
| `CANCELLED` | Runner explicitly cancelled the agent. | Only values from a completed measurement window may be reported; otherwise `null`. |
| `INTERNAL_ERROR` | Implementation failure. | Measurements are `null` unless independently completed and explicitly marked safe to retain. |

A status label does not replace raw metrics or source attribution. Status thresholds belong to the report layer and are configuration, not protocol truth.

## Capacity and MOS

MVP calculates neither generic capacity nor MOS.

A future configured-profile capacity estimate must document payload size, packetization interval, frame/transport-overhead assumptions, total packet rate, duration, impairment thresholds, and exact degradation rule. It must be labelled as an estimate for that named profile.

A future MOS calculation requires codec, packetization, delay and loss-impairment assumptions, formula/version, and validation. MOS without those inputs is prohibited.

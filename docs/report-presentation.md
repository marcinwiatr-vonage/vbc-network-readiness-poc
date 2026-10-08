# Report presentation contract

This document defines the presentation model for a controlled test result. It is intentionally separate from `proto/result.schema.json`: the result schema is the evidence contract, while this document defines how a future report page renders that evidence without inventing values or hiding unavailable observations.

The current local agent does not yet emit the full result contract and no web page is implemented. This document is the source of truth for that Milestone 6 implementation.

## Presentation rules

A report is a single-test evidence view, not a scorecard. The page must show the raw result and its interpretation together. It must never replace `null` with zero, omit the reason a measurement is unavailable, merge upstream and downstream values, or imply that a controlled probe represents a production media path.

The page header contains:

- status (`COMPLETED`, `UDP_UNREACHABLE_OR_BLOCKED`, `SESSION_REJECTED`, `CANCELLED`, or `INTERNAL_ERROR`);
- test ID, report schema version, protocol version, started-at and ended-at timestamps;
- selected probe ID, probe region, and the exact UDP port used;
- agent version, connection type, and profile name/parameters; and
- optional public-IP display, only when the runner explicitly enables it.

The duration is derived from the displayed timestamps only when both timestamps are valid. It is a display convenience, not a replacement for the profile's measurement duration.

## Required evidence layout

The report renders these sections in this order:

1. **Test metadata** — raw identifiers and versions, selected endpoint, profile, timestamps, and connection metadata.
2. **Reachability** — `UDP <port>` with exactly one of `bidirectional`, `no_validated_response`, or `not_tested`. The page must not show a port matrix or scan-like list.
3. **Uplink** — packets sent/received, duplicates, out-of-order packets, loss percentage, jitter p50/p95, throughput, and observer `probe`.
4. **Downlink** — the same fields, independently rendered, with observer `agent`.
5. **RTT** — min/mean/p95 in milliseconds. The label is `RTT`; never call it one-way latency.
6. **Conditions and warnings** — threshold-backed labels beside the raw values that caused them.

Every numeric field displays its unit. Recommended formatting is loss as `%`, jitter and RTT as `ms`, and throughput as `kbps`. Formatting may round for readability, but the raw JSON value must remain available via a details element, copy action, or equivalent accessible disclosure.

Each directional card visibly identifies its source of truth:

- `Uplink — observed by controlled probe`
- `Downlink — observed by endpoint agent`

No aggregate loss, jitter, or throughput value is allowed.

## Null and not-tested rendering

The renderer uses two distinct tokens:

- `null` → `Not available` with an explanation such as `No eligible authenticated samples` or `Measurement was not completed`.
- `not_tested` → `Not tested` with an explanation such as `Check was intentionally skipped`.

A null metric is never displayed as `0`, `0.0`, an empty cell, a green state, or a dash with no explanation. A not-tested field is never treated as a failure or success. For `UDP_UNREACHABLE_OR_BLOCKED`, all directional metrics and RTT are null and reachability is `no_validated_response`; for session rejection, cancellation, and internal error, reachability is `not_tested` unless a separate control-plane observation exists. These rules follow `docs/report-contract.md` and the result schema.

The page must remain useful for an unavailable result: it shows the attempted probe/region/port, status, timestamps, and the exact unavailable reason while clearly stating that no quality measurement was obtained.

## Threshold-backed conditions

Conditions are presentation labels, not protocol fields. A condition block is valid only when it includes all of the following:

```text
label:        uplink impairment
threshold:    loss_pct > 1.0%
threshold_version: readiness-defaults-v1
raw evidence: upstream.loss_pct = 2.5%
observer:     probe
```

The same shape is used for these initial labels:

| Label | Evidence | Example boundary in `readiness-defaults-v1` |
|---|---|---|
| `UDP unreachable` | `reachability.status` | `status == no_validated_response` |
| `uplink impairment` | `upstream.loss_pct` and/or `upstream.jitter_ms_p95` | loss `> 1.0%` or p95 jitter `> 30 ms` |
| `downlink impairment` | `downstream.loss_pct` and/or `downstream.jitter_ms_p95` | loss `> 1.0%` or p95 jitter `> 30 ms` |
| `variable latency` | `rtt_ms.p95` | p95 RTT `> 150 ms` |
| `insufficient configured-profile capacity` | profile-specific throughput evidence | not enabled until Milestone 7 defines and validates the profile rule |

The thresholds above are presentation defaults only; they do not alter the result schema or claim generic call capacity. A threshold set must be versioned and its exact comparison boundaries must be rendered. The raw evidence is shown even when a condition is not raised, for example `No uplink impairment — loss 0.0%, p95 jitter 1.2 ms (readiness-defaults-v1)`.

A null evidence value cannot raise a green or red metric condition. It renders as `Not evaluated — raw value null`, with the result status and unavailable reason shown instead. The capacity label remains `Not tested` until its assumptions and degradation rule are implemented and validated.

## Optional public IP

`public_ip_display` is runner-visible and opt-in. It is sourced only from the probe-observed source address; the client cannot supply it. The default report response excludes the raw address and the result store never persists it by default. If display is enabled, the UI must make the transient nature explicit and must not include the raw address in downloaded JSON, browser history, analytics, logs, or saved report URLs. A future history feature may use a masked value only after a documented retention policy.

## Accessibility and export

The report must expose status and condition text without relying on colour. Direction, observer, units, threshold version, and unavailable reasons must be readable by screen readers and remain present in a plain-text or JSON export. JSON export is the first supported export and must preserve JSON `null` values and `not_tested` reachability exactly. HTML/PDF export is deferred until the schema and presentation mapping are stable.

## Acceptance examples

A clean completed result shows `COMPLETED`, `bidirectional`, separate populated uplink/downlink cards, RTT values, and `No ... impairment` condition statements with raw evidence and the threshold version.

A directional-loss result shows an uplink impairment condition only when the uplink evidence crosses the configured boundary; downlink remains independently evaluated.

A no-probe-response result shows `UDP_UNREACHABLE_OR_BLOCKED`, the selected port, `no_validated_response`, null directional and RTT values, and no loss, jitter, throughput, or capacity condition fabricated from the absence of packets.

A session-rejected result shows the selected metadata where available, `SESSION_REJECTED`, `not_tested` reachability, null measurements, and no threshold evaluation.

# Threat Model

## Scope and assets

The system accepts untrusted traffic only through the HTTPS control plane and a project-controlled UDP listener on **`10000` or `16384`**. The canonical v2 frame is fixed-header binary data with a 32-byte HMAC-SHA256 tag. The first POC is `fra` / `eu-central-1`; later regions are separate deployments and trust boundaries.

Assets to protect:

- Availability, network reputation, and cost budget of each regional UDP probe.
- Integrity and attribution of directional measurements.
- Session IDs, one-time HMAC material, deployment credentials, and release artifacts.
- Endpoint privacy, including observed source address and aggregate result data.
- Terraform-managed infrastructure and its ingress configuration.

## Primary threats and required controls

| Threat | Risk | Required controls |
|---|---|---|
| UDP reflection or amplification | Probe sends traffic to a victim. | One-time authenticated session; atomic first-verified-source `IP:port` binding; no client-provided destination; one fixed response per admitted uplink; local hard ceilings of 32 admitted packets and two seconds; no echo; silence for unknown traffic. |
| Session replay | Reused material creates traffic or corrupts results. | Single-use test ID, short expiry, atomic state transition, HMAC, source binding, and replay tests. |
| Forged frame or result | False diagnostic conclusion. | Exact v2 framing, per-session HMAC-SHA256, strict schema/range validation, and separately attributed agent/probe observations. |
| Parser denial of service | Malformed input crashes the probe or consumes memory/CPU. | Reject frames under 68 bytes; validate magic/version/direction before session lookup; reject payload length over 1200 before allocation; require exact length up to 1268; verify HMAC; fuzz/error-path tests; unprivileged Rust containers. |
| Resource exhaustion | CPU, sockets, bandwidth, or spend exhaustion. | The local loop has a fixed 32-packet admission ceiling and a monotonic two-second duration ceiling; future public operation additionally requires per-source/session/concurrency creation and packet-rate limits, budgets, alerts, monitoring, and an ingress disable/destroy runbook. |
| Probe impersonation or target substitution | Agent sends authenticated data to an unintended target. | HTTPS-authenticated session response, explicit configured probe ID/host/port, agent allowlist/session binding, and no client-controlled UDP destination. |
| Sensitive data retention | Privacy or compliance exposure. | No customer data, credentials, payload logs, or persistent raw IP by default; minimise/hash where needed; documented retention/deletion policy. |
| Credential leakage | Infrastructure or source compromise. | No secrets in source/chat/logs, least privilege, short-lived roles where available, secret scanning, root MFA, and rotation/revocation procedure. |
| Local session-file disclosure or target substitution | One-time HMAC material leaks or the agent is redirected. | Strict 1 KiB versioned file parser; canonical lowercase hex; exact IPv4 loopback target; only UDP `10000` or `16384`; no credential output; operator creates the file outside the repository with restricted permissions and deletes it after use. |
| Terraform drift or manual exposure | Unknown ingress or recurring cost. | Terraform-managed permanent resources, reviewed plans, exact UDP ingress on `10000` or `16384`, state protection, cost budget, and destroy procedure. |
| Supply-chain compromise | Malicious dependency or artifact. | Dependency review, `cargo audit`, CI builds, release provenance/checksums, and prompt patching. |

## Mandatory abuse-case tests before public exposure

1. A random UDP datagram produces no response.
2. A validly framed packet with an unknown test ID produces no response.
3. Expired, replayed, invalid-HMAC, wrong-direction, and source-mismatched frames produce no response.
4. A session cannot direct probe traffic to an address other than the first validated endpoint source `IP:port`.
5. The local loop enforces its 32-admitted-packet and two-second ceilings. Packet-rate, concurrent-session, and creation-rate ceilings remain mandatory before public exposure.
6. Frames shorter than 68 bytes, larger than 1268 bytes, invalid magic/version/direction, payload lengths above 1200, and length mismatches are rejected without process crash or unbounded allocation.
7. The local probe rejects a non-loopback bind address, accepts only an explicitly supplied session ID/key and uplink direction, binds the first authenticated source atomically, admits only strictly increasing non-zero uplink sequences, and cannot accept a response destination from the client.
8. The agent accepts only authenticated downlink frames for its active session from the selected probe endpoint.
9. Result submission cannot overwrite another session or create a second authoritative result.
10. The externally provisioned UDP listener is exactly `10000` or `16384`; no unapproved listener is exposed, and unknown traffic remains silent from a non-local network.
11. Both local binaries reject oversized, malformed, non-canonical, reordered, or trailing session-file data and reject any target other than `127.0.0.1:10000` or `127.0.0.1:16384` before opening or sending on a UDP socket.

## Residual risk and operational response

A public UDP service always receives hostile traffic and consumes network capacity. The POC must stay low-volume, monitored, bounded, and disposable. If abuse, unexpected cost, excessive errors, or security uncertainty appears, disable UDP ingress, revoke active sessions, preserve only safe operational evidence, and destroy the Terraform-managed environment when appropriate.

## Security change procedure

Before changing UDP ingress, v2 frame/session format, identity/data collection, IAM, Terraform state, dependency class, or adding an optional marker protocol:

1. Update this threat model and the relevant architecture/report contract.
2. Add automated security and error-path tests, including no-response behavior.
3. Document detection, rollback, ingress disablement, and destroy steps.
4. Obtain explicit review before deployment.

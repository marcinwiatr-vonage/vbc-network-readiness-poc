# Threat Model

## Assets to protect

- Availability and network reputation of the regional UDP probe.
- Integrity of reported measurements.
- AWS account, IAM credentials and billing budget.
- Endpoint privacy: source address and aggregate metrics should be minimised and protected.
- Repository integrity and release artefacts.

## Primary threats and controls

| Threat | Risk | Required controls |
|---|---|---|
| UDP reflection/amplification | Probe can be abused to send traffic to victims. | One-time authenticated session, source binding, no client-provided destination, bounded response size/rate/duration, no echo. |
| Session replay | Reused token creates traffic or corrupts results. | Single-use test ID, short expiry, atomic state transition, HMAC and source binding. |
| Forged frames/results | False diagnostic conclusions. | Per-session HMAC, strict schema/range validation, server-side and agent-side observations stored separately. |
| Resource exhaustion | CPU, sockets, bandwidth or AWS spend exhaustion. | Per-IP/session/concurrency rate limits, small fixed packet caps, timeouts, cloud budget alarms, observability. |
| Protocol parser vulnerability | Probe compromise or crash from malformed input. | Fixed framing, length validation before allocation, fuzz tests, Rust ownership/memory safety, unprivileged containers. |
| Sensitive data retention | Privacy or compliance exposure. | No customer data/credentials/payload logs; hash/minimise source IP; retention and deletion policy. |
| Credential leakage | AWS/GitHub compromise. | No secrets in source/chat/logs, least privilege, OIDC/short-lived roles later, secret scanning, root MFA. |
| Terraform drift/click-ops | Unknown exposure or recurring cost. | Terraform-only permanent resources, reviewed plans, state protection, documented destroy procedure. |
| Supply-chain compromise | Malicious dependency/artifact. | Pinned dependencies, `cargo audit`, dependency review, CI builds, release provenance/checksums. |

## Abuse-case requirements

The following tests are mandatory before a public probe is exposed:

1. A random UDP datagram produces no response.
2. A validly formatted frame with unknown test ID produces no response.
3. Expired, replayed and invalid-HMAC frames produce no response.
4. A session cannot direct probe traffic to an address different from the validated endpoint.
5. Packet-rate, duration, payload-size and concurrent-session ceilings are enforced.
6. Malformed frames are rejected without process crash or unbounded memory allocation.
7. The local bootstrap probe rejects a non-loopback bind address, accepts only an explicitly supplied session ID/key and uplink direction, and cannot accept a client-provided response destination.
8. An agent accepts only an authenticated downlink response for its active session from the selected loopback probe address.
9. Result submission cannot overwrite a different session or create a second authoritative result.

## Residual risk

A public UDP service always receives hostile traffic and consumes network capacity. The POC must remain low-volume, monitored and immediately disposable. If abuse, unexpected cost, excessive error rate or security uncertainty appears, disable public ingress and destroy the Terraform-managed environment.

## Security change procedure

Before changing UDP ingress, session format, identity/data collection, IAM, Terraform state, dependency class or adding SIP/STUN/TURN/WebRTC:

1. Update this document with the new threat and control.
2. Add automated security/error-path tests.
3. Document rollback and operational detection.
4. Obtain explicit review before deployment.

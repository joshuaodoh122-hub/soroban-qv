# Security Policy

## Scope

This policy covers the `soroban-qv-core` contract and the `grant-vote` example.

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✅        |

## Reporting a vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Email: `joshuaodoh122@gmail.com` (replace with your actual address).

Please include:
- A description of the vulnerability and its impact.
- Steps to reproduce or a proof-of-concept (ideally a failing test).
- Your assessment of severity (critical / high / medium / low).

We will acknowledge receipt within 48 hours and aim to triage within 7 days.
We will credit reporters by name (or anonymously, at your preference) in the
CHANGELOG.

## Known limitations (not vulnerabilities)

The following are documented design decisions, not reportable issues:

- **No sybil resistance.** Any Stellar address can vote. See README Known
  Limitations.
- **Fixed credit allocation.** All voters receive equal credits. Token-weighted
  allocation is a planned future feature, not a current gap.
- **No time-based expiry.** Rounds close only when the admin calls `close_round`.

## Security properties the contract does guarantee

- A voter cannot spend more credits than their allocation (enforced on-chain).
- The quadratic cost cannot be gamed by splitting `cast_vote` calls — the
  marginal cost accounting prevents this explicitly.
- Only the round's own admin can close it — no other address can halt voting.
- All arithmetic uses checked operations; no silent overflow or underflow.
- Auth is enforced by Soroban's native framework — no custom signature handling.

## Audit status

This contract has **not been audited** as of v0.1.0. It is a boilerplate and
should be treated as such. Production use requires an independent security review.

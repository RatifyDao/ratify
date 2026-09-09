# Security Policy

RatifyDAO contracts handle governance decisions and community treasury funds.
Security disclosures are taken seriously and handled with priority.

---

## Scope

The following are in scope for security reports:

| Component | Examples |
|---|---|
| Soroban contracts | Logic errors, access control bypasses, integer overflow, reentrancy |
| Indexer | SQL injection, event replay attacks, data corruption |
| Web interface | XSS, transaction manipulation, wallet connection exploits |
| Shared library | Type confusion, address validation bypass |

The following are **out of scope**:

- Issues in Stellar core or the Soroban host runtime (report to SDF)
- Issues in third-party dependencies (report upstream)
- Theoretical issues with no practical exploit path
- UI cosmetic issues

---

## Reporting a vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Send a report to: **security@ratifydao.xyz**

Include:

1. A clear description of the vulnerability.
2. The affected component and version / commit hash.
3. Steps to reproduce or a proof-of-concept.
4. Your assessment of the impact.
5. Any suggested fix, if you have one.

You will receive an acknowledgement within **48 hours** and a full response
within **7 days**.

---

## Disclosure policy

- We follow coordinated disclosure. Please give us reasonable time to fix the
  issue before making it public.
- We will credit reporters in the release notes unless you prefer to remain
  anonymous.
- There is currently no bug bounty programme. This will be revisited after a
  mainnet deployment.

---

## Audit status

See [AUDIT.md](AUDIT.md) for the current audit status of each contract.

**The contracts have not yet been audited for mainnet deployment. Do not hold
significant funds in a RatifyDAO treasury on mainnet until an audit has been
completed.**

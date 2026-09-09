# Deployment

Deployed contract addresses for every network, alongside the settings each
community was configured with.

---

## Testnet

Network: `Test SDF Network ; September 2015`
RPC: `https://soroban-testnet.stellar.org`
Deployed: 2026-09-09

### Protocol contracts

| Contract | Address |
|---|---|
| `ratify-factory` | `CBTXUGABEUSSOZCYQ5LEMLK2FXGK5EIEOO6S23R4MHPEUVZ5ETOFCJO2` |
| `ratify-delegate-registry` | `CBW7F2UDNJW3FZZDKCQXXYYMO3SJWKMDHEKPA3CLUECBFJNNF3TGC5AA` |

### Community: Riverside Commons (demo)

| Contract | Address |
|---|---|
| `ratify-governor` | `CA2KM2INEXMFERSTWLR5P2ZQ7EOB2K72C3HUST7G6FSTQDACRXRPXEEE` |
| `ratify-membership` | `CBCOK7HYPK5FQ4LWZUJRIKONFGRTRGFAR7RMTDODDZ4LULU5GCXFWLBG` |
| `ratify-weight-rule` | `CBJMAT77Z3KSD4XI6TJOYNECRP65LM7FSRT744H2WLS6N2ZNLEFBUOKV` |
| `ratify-timelock` | `CBOOKX3CYNKEJEBTEHUQBHXDDGG6IO7ZF5AFQ3CQHZFQZJTBQLNAUUEK` |
| `ratify-treasury` | `CAOZGSGMH6DAAJ4YVTKCPNVTQQU4O4PULRG43NCYY4CQFZUQTO77NYLB` |
| `ratify-registry` | `CBW7F2UDNJW3FZZDKCQXXYYMO3SJWKMDHEKPA3CLUECBFJNNF3TGC5AA` |

### Demo community settings

| Setting | Value |
|---|---|
| Voting delay | 2 ledgers (~10s) |
| Voting period | 24 ledgers (~2m) |
| Timelock delay | 12 ledgers (~1m) |
| Quorum | 20% of live voting power |
| Weight model | One token one vote |

### Verified activity

| Action | Transaction |
|---|---|
| Membership issued | on-chain |
| Voting power granted | on-chain |
| Spending policy set by vote | on-chain |
| Payment of 4 XLM made by vote | on-chain |

---

## Mainnet

Not yet deployed. See [AUDIT.md](AUDIT.md) for prerequisites.

---

## Deploying a new community

```bash
# 1. Compile contracts
stellar contract build

# 2. Deploy via the factory (one transaction)
node packages/indexer/scripts/seed-testnet.mjs deploy

# 3. Update this file with the new addresses
# 4. Update packages/shared/src/addresses.ts
# 5. Run the indexer against the new factory
RATIFY_FACTORY_ID=<new_factory> npm run indexer
```

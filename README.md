# 🔐 Age Verify — Soroban Smart Contract

> Prove you're old enough. Never reveal how old you actually are.

A privacy-preserving age-gate contract built on **Stellar's Soroban** platform. Verifies that a user meets an age requirement — 18+, 21+, or any threshold — **without ever writing their birthdate to the blockchain.**

---

## Project Description

Age verification is a legal requirement for countless digital products: alcohol marketplaces, gambling platforms, financial services, adult content, and more. The naive blockchain approach — storing a birthdate and doing the maths on-chain — turns a private fact into permanently public data. Anyone who can read chain state can read your birthday, forever.

**Age Verify** solves this with a two-phase model:

1. **Off-chain** — A trusted verifier (your backend, a KYC provider, or a DAO multisig) checks the user's real ID document and computes `SHA-256(birthdate || random_salt)`.
2. **On-chain** — Only the hash and a boolean result (`meets_requirement: true/false`) are written to Soroban's persistent storage. The raw birthdate never appears anywhere on the ledger.

Any dApp can then call `is_old_enough(user, 18)` and get a trustworthy yes/no without ever touching personal data.

---

## What It Does

### 1 · Admin Initialization
Deploy the contract and designate one address (a backend wallet, multisig, or DAO) as the **admin** — the only entity authorised to write verification records.

### 2 · Off-Chain Verification → On-Chain Record
After a user consents and the verifier checks their government ID off-chain:

```
birthdate_hash = SHA-256( "1994-03-22" || "<random 32-byte salt>" )
```

The admin calls `record_verification(user, birthdate_hash, meets_req, required_age)`.  
The contract stores:
- The hash (for auditability — can be re-verified off-chain without re-uploading the date)
- `meets_requirement` — did they pass the check?
- `required_age` — which threshold was tested (e.g. 18)
- `verified_at_ledger` — Stellar ledger sequence for timestamping

**No birthdate. No age. No personally identifiable information.**

### 3 · dApp Query
Any contract or frontend can ask:

```rust
is_old_enough(user_address, 18)   // → true / false
has_record(user_address)          // → true / false
get_record(user_address)          // → AgeRecord { hash, meets_req, required_age, ledger }
```

### 4 · Revocation
Users can request deletion; admin calls `revoke(user)` to wipe the record from storage and emit a `revoked` event.

### 5 · Admin Transfer
Admin key can be rotated at any time with `set_admin(new_admin)` — supports migration to a multisig or governance contract.

---

## Features

| Feature | Detail |
|---|---|
| **Zero birthdate on-chain** | Only a salted SHA-256 hash and a boolean are persisted |
| **Threshold-flexible** | Record one age check, query any threshold ≤ that value |
| **Minimal storage footprint** | One `AgeRecord` struct per user in Soroban persistent storage |
| **Event emission** | `verified` and `revoked` events for off-chain indexers and frontends |
| **Revocable records** | GDPR / right-to-erasure friendly — admin can delete any record |
| **Admin rotation** | Safely hand off the verifier role to a multisig or DAO |
| **Double-init protection** | `initialize()` panics if called more than once |
| **Test suite included** | 6 unit tests covering happy path, failure, revocation, and edge cases |

---

## Privacy Model

```
User's real ID
      │
      ▼  (off-chain only)
Trusted Verifier
      │  computes SHA-256(birthdate ∥ salt)
      │  checks  age ≥ required_age
      │
      ▼  (written to chain)
┌─────────────────────────────────┐
│  AgeRecord                      │
│  ├─ birthdate_hash : Hash<32>   │  ← hash, not the date
│  ├─ meets_requirement : bool    │  ← yes/no, not the age
│  ├─ required_age : u32          │  ← threshold, not the age
│  └─ verified_at_ledger : u32    │
└─────────────────────────────────┘
      │
      ▼  (queried by dApps)
is_old_enough(user, 18) → true
```

The raw birthdate exists only on the verifier's secured backend. Even if the entire Stellar ledger history were publicly analysed, no birthdate could be extracted.

---

## Project Structure

```
age-verify/
├── Cargo.toml          # Soroban SDK dependency, release profile
└── src/
    └── lib.rs          # Contract code + unit tests
```

---

## Getting Started

### Prerequisites

```bash
# Rust + wasm32 target
rustup target add wasm32-unknown-unknown

# Soroban CLI
cargo install --locked soroban-cli
```

### Build

```bash
cd age-verify
cargo build --target wasm32-unknown-unknown --release
```

The compiled `.wasm` lives at:
`target/wasm32-unknown-unknown/release/age_verify.wasm`

### Test

```bash
cargo test
```

### Deploy to Testnet

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/age_verify.wasm \
  --source <YOUR_SECRET_KEY> \
  --rpc-url https://soroban-testnet.stellar.org \
  --network-passphrase "Test SDF Network ; September 2015"
```

### Invoke

```bash
# Initialize
soroban contract invoke --id <CONTRACT_ID> \
  -- initialize --admin <ADMIN_ADDRESS>

# Record a verification (admin signs)
soroban contract invoke --id <CONTRACT_ID> \
  -- record_verification \
     --user <USER_ADDRESS> \
     --birthdate_hash <HEX_HASH> \
     --meets_req true \
     --required_age 18

# Query
soroban contract invoke --id <CONTRACT_ID> \
  -- is_old_enough --user <USER_ADDRESS> --min_age 18
```

---

## Security Considerations

- **Salt your hashes.** Without a salt, `SHA-256("1990-01-01")` is deterministic — an attacker with a list of likely birthdates can brute-force the hash. Use a unique, high-entropy salt per user stored securely off-chain.
- **Protect the admin key.** The admin address is the single point of trust for writing records. In production, use a multisig or a Soroban auth policy contract.
- **The boolean doesn't encode the exact age.** `required_age` stores the *threshold checked*, not the user's actual age — there is no way to derive the exact age from the on-chain data.
- **Ledger data is permanent.** Even after `revoke()` removes the live record, archival nodes may retain historical state. Inform users accordingly.

---

## Roadmap

- [ ] Multi-threshold records — store multiple verified thresholds in one record
- [ ] ZK-proof variant — replace the trusted-admin model with a zero-knowledge proof for fully trustless verification
- [ ] Expiry / TTL — auto-expire records after N ledgers to enforce re-verification
- [ ] Cross-contract interface — publish a standard trait so any Soroban dApp can plug in this verifier

---

## License

MIT — see `LICENSE` for details.

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype,
    symbol_short,
    Address, Bytes, BytesN, Env, Symbol,
};

// ── Storage keys ──────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");

/// Persistent record stored per user.
/// We only keep:
///   • a salted hash of the birthdate  (so the raw date never hits the chain)
///   • the boolean result: is the user ≥ the threshold that was checked?
#[contracttype]
#[derive(Clone)]
pub struct AgeRecord {
    /// SHA-256 hash of (birthdate_iso || salt)
    /// Computed off-chain; stored here only for auditability.
    /// BytesN<32> is used instead of Hash<32> for full contracttype compatibility.
    pub birthdate_hash: BytesN<32>,
    /// True  ⟹ the off-chain verifier confirmed age ≥ required_age
    pub meets_requirement: bool,
    /// The minimum age that was checked (e.g. 18).
    pub required_age: u32,
    /// Ledger sequence number when the record was written.
    pub verified_at_ledger: u32,
}

#[contracttype]
pub enum DataKey {
    /// AgeRecord keyed by the user's Stellar address.
    Record(Address),
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct AgeVerifierContract;

#[contractimpl]
impl AgeVerifierContract {
    // ── Admin / setup ─────────────────────────────────────────────────────

    /// Deploy-time initialisation. Must be called exactly once.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialised");
        }
        env.storage().instance().set(&ADMIN, &admin);
    }

    /// Replace the admin. Requires the current admin to sign.
    pub fn set_admin(env: Env, new_admin: Address) {
        let current: Address = env.storage().instance().get(&ADMIN).unwrap();
        current.require_auth();
        env.storage().instance().set(&ADMIN, &new_admin);
        env.events().publish(
            (symbol_short!("set_admin"),),
            new_admin,
        );
    }

    // ── Core verification logic ───────────────────────────────────────────

    /// Called by the admin after performing the real-world age check off-chain.
    ///
    /// Parameters
    /// ----------
    /// user            – the Stellar address being verified
    /// birthdate_hash  – SHA-256( birthdate_iso_string || random_salt )
    ///                   computed off-chain; the raw birthdate NEVER appears here
    /// meets_req       – did the user satisfy the required age?
    /// required_age    – the threshold that was checked (e.g. 18 or 21)
    pub fn record_verification(
        env: Env,
        user: Address,
        birthdate_hash: BytesN<32>,
        meets_req: bool,
        required_age: u32,
    ) {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        admin.require_auth();

        let record = AgeRecord {
            birthdate_hash,
            meets_requirement: meets_req,
            required_age,
            verified_at_ledger: env.ledger().sequence(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Record(user.clone()), &record);

        // Emit a minimal event — no birthdate, no exact age.
        env.events().publish(
            (symbol_short!("verified"), user),
            (meets_req, required_age),
        );
    }

    // ── Query helpers ─────────────────────────────────────────────────────

    /// Returns `true` if the user was verified for *at least* `min_age`.
    pub fn is_old_enough(env: Env, user: Address, min_age: u32) -> bool {
        match env
            .storage()
            .persistent()
            .get::<DataKey, AgeRecord>(&DataKey::Record(user))
        {
            Some(r) => r.meets_requirement && r.required_age >= min_age,
            None => false,
        }
    }

    /// Returns the full AgeRecord for a user, or panics if not found.
    pub fn get_record(env: Env, user: Address) -> AgeRecord {
        env.storage()
            .persistent()
            .get::<DataKey, AgeRecord>(&DataKey::Record(user))
            .expect("no record found for user")
    }

    /// Returns `true` if any verification record exists for the user.
    pub fn has_record(env: Env, user: Address) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Record(user))
    }

    /// Admin can remove a record (e.g. on user request / GDPR-style deletion).
    pub fn revoke(env: Env, user: Address) {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        admin.require_auth();
        env.storage()
            .persistent()
            .remove(&DataKey::Record(user.clone()));
        env.events().publish(
            (symbol_short!("revoked"), user),
            symbol_short!("removed"),
        );
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (Env, AgeVerifierContractClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register_contract(None, AgeVerifierContract);
        let client = AgeVerifierContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let user  = Address::generate(&env);

        client.initialize(&admin);
        (env, client, admin, user)
    }

    fn dummy_hash(env: &Env) -> BytesN<32> {
        // sha256 of 64 zero bytes — stand-in for real off-chain hash
        env.crypto().sha256(&Bytes::from_array(env, &[0u8; 64]))
    }

    #[test]
    fn test_verify_adult_pass() {
        let (env, client, _admin, user) = setup();
        let h = dummy_hash(&env);

        client.record_verification(&user, &h, &true, &18);

        assert!(client.is_old_enough(&user, &18));
        assert!(client.is_old_enough(&user, &16)); // 18+ satisfies 16+
        assert!(!client.is_old_enough(&user, &21)); // 18+ doesn't satisfy 21+
    }

    #[test]
    fn test_verify_adult_fail() {
        let (env, client, _admin, user) = setup();
        let h = dummy_hash(&env);

        client.record_verification(&user, &h, &false, &18);

        assert!(!client.is_old_enough(&user, &18));
    }

    #[test]
    fn test_no_record_returns_false() {
        let (_env, client, _admin, user) = setup();
        assert!(!client.is_old_enough(&user, &18));
        assert!(!client.has_record(&user));
    }

    #[test]
    fn test_revoke() {
        let (env, client, _admin, user) = setup();
        let h = dummy_hash(&env);

        client.record_verification(&user, &h, &true, &18);
        assert!(client.has_record(&user));

        client.revoke(&user);
        assert!(!client.has_record(&user));
        assert!(!client.is_old_enough(&user, &18));
    }

    #[test]
    fn test_get_record_fields() {
        let (env, client, _admin, user) = setup();
        let h = dummy_hash(&env);

        client.record_verification(&user, &h, &true, &21);
        let rec = client.get_record(&user);

        assert_eq!(rec.meets_requirement, true);
        assert_eq!(rec.required_age, 21);
        assert_eq!(rec.birthdate_hash, h);
    }

    #[test]
    #[should_panic(expected = "already initialised")]
    fn test_double_init_panics() {
        let (_env, client, admin, _user) = setup();
        client.initialize(&admin);
    }
}
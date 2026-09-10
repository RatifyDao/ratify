#![no_std]
//! # Ratify membership
//!
//! Membership tokens, and voting power that lapses unless it is renewed.
//!
//! Two decisions shape this contract.
//!
//! **Membership is a credential, not an asset.** Tokens cannot be
//! transferred, sold or approved to anyone. A vote that can be bought is not
//! a vote, and every weighting model in RatifyDAO exists to avoid rule by
//! whoever is willing to spend the most.
//!
//! **Voting power carries a term.** An account holding tokens has no power
//! until it grants that power, and the grant expires. A member can grant to
//! themselves, which is the ordinary case, or to a delegate. Either way the
//! grant lapses unless it is renewed, and lapsed power stops counting for the
//! delegate and for quorum both.
//!
//! ## Why the vote accounting is written here
//!
//! The OpenZeppelin Stellar votes module is the natural place to get
//! checkpointed voting power, and RatifyDAO uses OpenZeppelin for the token
//! itself. But its delegation is permanent until the delegator signs again,
//! and undoing one requires the delegator's authorisation. A grant that
//! lapses on its own cannot be built on that, because at the moment it lapses
//! there is nobody to sign. So the power accounting is RatifyDAO's own, and it is
//! the smaller half of the contract: a checkpoint series per delegate, plus
//! one for the live total.
//!
//! ## Sweeping
//!
//! An expired grant stops counting the moment [`RatifyMembership::lapse`] is
//! called for it, and anyone may call that for anyone. It is not a privilege
//! and it costs the caller nothing but the fee. The indexer sweeps
//! continuously, so in practice the live total tracks reality; the guarantee
//! the contract makes is that no expired grant can be counted once it has been
//! swept, and that nobody can prevent a sweep.

mod checkpoint;
mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, panic_with_error, Address, Env, String};
use stellar_tokens::non_fungible::Base;

use crate::checkpoint::Series;
pub use crate::checkpoint::{Checkpoint, CheckpointKey};
pub use crate::types::{
    Grant, GrantLapsed, GrantMade, GrantRenewed, GrantWithdrawn, IssuerChanged, MembershipError,
    MembershipIssued, MembershipRevoked, MembershipStorageKey,
};

/// The longest term a grant may carry, in ledgers. Two years at five seconds
/// a ledger.
///
/// A term is a promise to come back. One that runs longer than this is a
/// permanent delegation wearing a costume.
pub const MAX_TERM_LEDGERS: u32 = 2 * 365 * 17_280;

#[contract]
pub struct RatifyMembership;

#[contractimpl]
impl RatifyMembership {
    /// Creates the membership token.
    ///
    /// The issuer is whoever may hand out and revoke membership. A community
    /// deploying through the factory starts with its founder here and moves
    /// issuance to the timelock with
    /// [`transfer_issuance`](RatifyMembership::transfer_issuance), after which
    /// admitting a member is a proposal like anything else.
    pub fn __constructor(e: &Env, issuer: Address, name: String, symbol: String, base_uri: String) {
        e.storage()
            .instance()
            .set(&MembershipStorageKey::Issuer, &issuer);
        e.storage()
            .instance()
            .set(&MembershipStorageKey::MemberCount, &0u32);
        Base::set_metadata(e, base_uri, name, symbol);
    }

    // ################## THE TOKEN ##################

    /// Returns the name of the membership token.
    pub fn name(e: &Env) -> String {
        Base::name(e)
    }

    /// Returns the symbol of the membership token.
    pub fn symbol(e: &Env) -> String {
        Base::symbol(e)
    }

    /// Returns the metadata URI for one token.
    pub fn token_uri(e: &Env, token_id: u32) -> String {
        Base::token_uri(e, token_id)
    }

    /// Returns how many membership tokens an account holds.
    pub fn balance(e: &Env, account: Address) -> u32 {
        Base::balance(e, &account)
    }

    /// Returns the holder of a token.
    pub fn owner_of(e: &Env, token_id: u32) -> Address {
        Base::owner_of(e, token_id)
    }

    /// Returns the number of accounts holding at least one token.
    ///
    /// This is the denominator for one member one vote, and the figure the
    /// directory shows next to turnout.
    pub fn member_count(e: &Env) -> u32 {
        e.storage()
            .instance()
            .get(&MembershipStorageKey::MemberCount)
            .unwrap_or(0)
    }

    /// Returns the address that may issue and revoke membership.
    pub fn issuer(e: &Env) -> Address {
        e.storage()
            .instance()
            .get(&MembershipStorageKey::Issuer)
            .unwrap_or_else(|| panic_with_error!(e, MembershipError::IssuerNotSet))
    }

    /// Issues a membership token and returns its identifier.
    ///
    /// Issuing does not give the new member any voting power. They grant it
    /// themselves, for a term, which is the first act of turning up.
    pub fn issue(e: &Env, to: Address) -> u32 {
        Self::require_issuer(e);
        if Base::balance(e, &to) == 0 {
            Self::count_member(e, &to);
        }
        let token_id = Base::sequential_mint(e, &to);
        Self::sync_grant(e, &to);
        MembershipIssued { to, token_id }.publish(e);
        token_id
    }

    /// Revokes a membership token.
    ///
    /// Any voting power the token was carrying leaves with it, including power
    /// the holder had granted to a delegate.
    pub fn revoke(e: &Env, token_id: u32) {
        Self::require_issuer(e);
        let from = Base::owner_of(e, token_id);
        Base::update(e, Some(&from), None, token_id);
        Self::sync_grant(e, &from);
        if Base::balance(e, &from) == 0 {
            Self::discount_member(e, &from);
        }
        MembershipRevoked { from, token_id }.publish(e);
    }

    /// Hands the right to issue membership to another address.
    ///
    /// A community moves this to its timelock once it is running, so that
    /// admitting or removing a member becomes a proposal that waits out the
    /// delay like a payment.
    pub fn transfer_issuance(e: &Env, new_issuer: Address) {
        let old_issuer = Self::issuer(e);
        old_issuer.require_auth();
        e.storage()
            .instance()
            .set(&MembershipStorageKey::Issuer, &new_issuer);
        IssuerChanged {
            old_issuer,
            new_issuer,
        }
        .publish(e);
    }

    // ################## GRANTS ##################

    /// Returns the grant an account has made, if it has one.
    ///
    /// A grant is returned whether or not it has expired, so the membership
    /// page can show a member that theirs has lapsed and offer to renew it.
    pub fn grant(e: &Env, account: Address) -> Option<Grant> {
        e.storage()
            .persistent()
            .get(&MembershipStorageKey::Grant(account))
    }

    /// Returns whether an account's grant is still live.
    pub fn is_live(e: &Env, account: Address) -> bool {
        match Self::grant(e, account) {
            Some(grant) => grant.expires_at > e.ledger().sequence(),
            None => false,
        }
    }

    /// Returns how many ledgers remain before an account's grant lapses.
    ///
    /// Zero means it has lapsed, or was never made.
    pub fn ledgers_until_lapse(e: &Env, account: Address) -> u32 {
        match Self::grant(e, account) {
            Some(grant) => grant.expires_at.saturating_sub(e.ledger().sequence()),
            None => 0,
        }
    }

    /// Grants an account's voting power for a term.
    ///
    /// Pass the account's own address to hold the power yourself, which is
    /// what most members do. Passing someone else's makes them your delegate
    /// until the term ends.
    ///
    /// Granting again replaces the existing grant, moving the power at once.
    pub fn delegate_for(e: &Env, account: Address, delegatee: Address, term_ledgers: u32) {
        account.require_auth();

        if term_ledgers == 0 {
            panic_with_error!(e, MembershipError::TermCannotBeZero);
        }
        if term_ledgers > MAX_TERM_LEDGERS {
            panic_with_error!(e, MembershipError::TermTooLong);
        }

        let units = Base::balance(e, &account) as u128;
        if units == 0 {
            panic_with_error!(e, MembershipError::NoVotingUnits);
        }

        let now = e.ledger().sequence();
        let expires_at = now.saturating_add(term_ledgers);

        // Take the power back from wherever it currently sits, if anywhere.
        if let Some(existing) = Self::grant(e, account.clone()) {
            if existing.delegatee == delegatee && existing.expires_at == expires_at {
                panic_with_error!(e, MembershipError::AlreadyDelegated);
            }
            // A grant that has expired but not yet been swept is still
            // counted, so its power comes off here either way.
            Self::remove_power(e, &existing.delegatee, existing.units, true);
        }

        Self::add_power(e, &delegatee, units, true);
        Self::write_grant(e, &account, &delegatee, units, expires_at, now);

        GrantMade {
            account,
            delegatee,
            units,
            expires_at,
        }
        .publish(e);
    }

    /// Extends a live grant for a further term, without changing who holds it.
    ///
    /// This is the action the membership page prompts for as a term nears its
    /// end. A grant that has already lapsed cannot be renewed; it is made
    /// again, which is the same work and a clearer record.
    pub fn renew(e: &Env, account: Address, term_ledgers: u32) {
        account.require_auth();

        if term_ledgers == 0 {
            panic_with_error!(e, MembershipError::TermCannotBeZero);
        }
        if term_ledgers > MAX_TERM_LEDGERS {
            panic_with_error!(e, MembershipError::TermTooLong);
        }

        let grant = Self::grant(e, account.clone())
            .unwrap_or_else(|| panic_with_error!(e, MembershipError::NoGrant));

        let now = e.ledger().sequence();
        if grant.expires_at <= now {
            panic_with_error!(e, MembershipError::NoGrant);
        }

        let expires_at = now.saturating_add(term_ledgers);
        Self::write_grant(e, &account, &grant.delegatee, grant.units, expires_at, now);

        GrantRenewed {
            account,
            delegatee: grant.delegatee,
            expires_at,
        }
        .publish(e);
    }

    /// Takes back a live grant before its term is up.
    pub fn withdraw(e: &Env, account: Address) {
        account.require_auth();

        let grant = Self::grant(e, account.clone())
            .unwrap_or_else(|| panic_with_error!(e, MembershipError::NoGrant));

        let units = grant.units;
        Self::remove_power(e, &grant.delegatee, units, true);
        e.storage()
            .persistent()
            .remove(&MembershipStorageKey::Grant(account.clone()));

        GrantWithdrawn {
            account,
            delegatee: grant.delegatee,
            units,
        }
        .publish(e);
    }

    /// Sweeps an expired grant, taking its power out of the live total.
    ///
    /// Anyone may call this for anyone. It needs no authorisation, because the
    /// member already gave it when they chose a term. Nobody can stop a sweep,
    /// and nobody gains anything by performing one, which is what makes the
    /// lapse enforceable rather than a matter of good manners.
    pub fn lapse(e: &Env, account: Address) {
        let grant = Self::grant(e, account.clone())
            .unwrap_or_else(|| panic_with_error!(e, MembershipError::NoGrant));

        if grant.expires_at > e.ledger().sequence() {
            panic_with_error!(e, MembershipError::GrantStillLive);
        }

        let units = grant.units;
        Self::remove_power(e, &grant.delegatee, units, true);
        e.storage()
            .persistent()
            .remove(&MembershipStorageKey::Grant(account.clone()));

        GrantLapsed {
            account,
            delegatee: grant.delegatee,
            units,
            expired_at: grant.expires_at,
        }
        .publish(e);
    }

    // ################## VOTING POWER ##################

    /// Returns the voting power currently answering to an account.
    ///
    /// This counts power granted to it by others as well as its own, and
    /// excludes anything that has been swept.
    pub fn votes(e: &Env, account: Address) -> u128 {
        checkpoint::latest(e, &Series::Power(account))
    }

    /// Returns the voting power that answered to an account at a past ledger.
    ///
    /// This is what decides a vote, taken at the proposal's snapshot, so power
    /// acquired after a proposal opened cannot change its outcome.
    pub fn votes_at(e: &Env, account: Address, ledger: u32) -> u128 {
        checkpoint::value_at(e, &Series::Power(account), ledger)
    }

    /// Returns the community's live voting power.
    ///
    /// Direct holders plus delegations that have not lapsed. This is the
    /// denominator a quorum is measured against, which is why a quorum in
    /// RatifyDAO means something a quorum measured against every token ever
    /// issued does not.
    pub fn live_total(e: &Env) -> u128 {
        checkpoint::latest(e, &Series::LiveTotal)
    }

    /// Returns the community's live voting power at a past ledger.
    pub fn live_total_at(e: &Env, ledger: u32) -> u128 {
        checkpoint::value_at(e, &Series::LiveTotal, ledger)
    }

    /// Returns how many members' live grants currently answer to an account.
    ///
    /// One member one vote counts from here rather than from tokens, so
    /// holding ten membership tokens still gives one vote.
    pub fn heads(e: &Env, account: Address) -> u128 {
        checkpoint::latest(e, &Series::Heads(account))
    }

    /// Returns how many members' grants answered to an account at a past
    /// ledger.
    pub fn heads_at(e: &Env, account: Address, ledger: u32) -> u128 {
        checkpoint::value_at(e, &Series::Heads(account), ledger)
    }

    /// Returns how many members across the community hold a live grant.
    pub fn live_heads(e: &Env) -> u128 {
        checkpoint::latest(e, &Series::LiveHeads)
    }

    /// Returns how many members held a live grant at a past ledger.
    pub fn live_heads_at(e: &Env, ledger: u32) -> u128 {
        checkpoint::value_at(e, &Series::LiveHeads, ledger)
    }

    /// Returns the ledger an account became a member on, if it is one.
    ///
    /// Set when the first token arrives and cleared when the last one
    /// leaves, so it measures unbroken membership. Time weighted voting reads
    /// it, and it cannot be moved by acquiring or shedding further tokens.
    pub fn member_since(e: &Env, account: Address) -> Option<u32> {
        e.storage()
            .persistent()
            .get(&MembershipStorageKey::MemberSince(account))
    }

    /// Returns how many ledgers an account has been a member for, measured at
    /// `ledger`.
    ///
    /// Zero if they were not a member then, which is what stops a new member
    /// from claiming tenure on an old proposal.
    pub fn tenure_at(e: &Env, account: Address, ledger: u32) -> u32 {
        match Self::member_since(e, account) {
            Some(since) if since <= ledger => ledger - since,
            _ => 0,
        }
    }

    // ################## INTERNAL ##################

    fn require_issuer(e: &Env) {
        Self::issuer(e).require_auth();
    }

    fn write_grant(
        e: &Env,
        account: &Address,
        delegatee: &Address,
        units: u128,
        expires_at: u32,
        granted_at: u32,
    ) {
        let key = MembershipStorageKey::Grant(account.clone());
        let grant = Grant {
            delegatee: delegatee.clone(),
            units,
            expires_at,
            granted_at,
        };
        e.storage().persistent().set(&key, &grant);
        e.storage()
            .persistent()
            .extend_ttl(&key, 29 * 17_280, 30 * 17_280);
    }

    /// Adds a grant's weight to a delegate.
    ///
    /// `head` says whether this is a new grant rather than a resize, because a
    /// member counts once towards one member one vote however many tokens
    /// they hold.
    fn add_power(e: &Env, delegatee: &Address, units: u128, head: bool) {
        checkpoint::adjust(e, &Series::Power(delegatee.clone()), units as i128);
        checkpoint::adjust(e, &Series::LiveTotal, units as i128);
        if head {
            checkpoint::adjust(e, &Series::Heads(delegatee.clone()), 1);
            checkpoint::adjust(e, &Series::LiveHeads, 1);
        }
    }

    /// Takes a grant's weight off a delegate.
    fn remove_power(e: &Env, delegatee: &Address, units: u128, head: bool) {
        checkpoint::adjust(e, &Series::Power(delegatee.clone()), -(units as i128));
        checkpoint::adjust(e, &Series::LiveTotal, -(units as i128));
        if head {
            checkpoint::adjust(e, &Series::Heads(delegatee.clone()), -1);
            checkpoint::adjust(e, &Series::LiveHeads, -1);
        }
    }

    /// Keeps a grant in step with the account's holding after a token is
    /// issued or revoked.
    ///
    /// The power counted for a live grant is always the tokens the account
    /// holds now, so gaining a token adds to the delegate immediately and
    /// losing one takes it away.
    ///
    /// A grant that has already expired is swept here instead of resized. It
    /// was going to be swept anyway, and letting it grow on the way would let
    /// a delegate gain weight from a member who has stopped renewing.
    fn sync_grant(e: &Env, account: &Address) {
        let Some(grant) = Self::grant(e, account.clone()) else {
            return;
        };

        if grant.expires_at <= e.ledger().sequence() {
            Self::remove_power(e, &grant.delegatee, grant.units, true);
            e.storage()
                .persistent()
                .remove(&MembershipStorageKey::Grant(account.clone()));
            GrantLapsed {
                account: account.clone(),
                delegatee: grant.delegatee,
                units: grant.units,
                expired_at: grant.expires_at,
            }
            .publish(e);
            return;
        }

        let held = Base::balance(e, account) as u128;
        if held == grant.units {
            return;
        }

        if held > grant.units {
            Self::add_power(e, &grant.delegatee, held - grant.units, false);
        } else {
            Self::remove_power(e, &grant.delegatee, grant.units - held, false);
        }
        Self::write_grant(
            e,
            account,
            &grant.delegatee,
            held,
            grant.expires_at,
            grant.granted_at,
        );
    }

    fn count_member(e: &Env, account: &Address) {
        let key = MembershipStorageKey::Counted(account.clone());
        if e.storage()
            .persistent()
            .get::<_, bool>(&key)
            .unwrap_or(false)
        {
            return;
        }
        e.storage().persistent().set(&key, &true);
        e.storage().persistent().set(
            &MembershipStorageKey::MemberSince(account.clone()),
            &e.ledger().sequence(),
        );
        let count = Self::member_count(e);
        e.storage()
            .instance()
            .set(&MembershipStorageKey::MemberCount, &(count + 1));
    }

    fn discount_member(e: &Env, account: &Address) {
        let key = MembershipStorageKey::Counted(account.clone());
        if !e
            .storage()
            .persistent()
            .get::<_, bool>(&key)
            .unwrap_or(false)
        {
            return;
        }
        e.storage().persistent().remove(&key);
        e.storage()
            .persistent()
            .remove(&MembershipStorageKey::MemberSince(account.clone()));
        let count = Self::member_count(e);
        e.storage()
            .instance()
            .set(&MembershipStorageKey::MemberCount, &count.saturating_sub(1));
    }
}

//! Grants, storage keys, errors and events for the membership token.

use soroban_sdk::{contracterror, contractevent, contracttype, Address};

/// A grant of voting power, and the ledger at which it lapses.
///
/// Every grant has a term, including a member holding their own power. That is
/// the point: a quorum should be measured against members who confirmed
/// recently, not against everyone who ever joined.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Grant {
    /// Who the power answers to. May be the member themselves.
    pub delegatee: Address,
    /// The units this grant has counted for its delegate. Kept on the grant
    /// so issuing or revoking a token can adjust exactly what was counted.
    pub units: u128,
    /// The ledger from which the grant no longer counts.
    pub expires_at: u32,
    /// The ledger the grant was last made or renewed on. Shown on the
    /// membership page so a member can see how long they have had it.
    pub granted_at: u32,
}

#[contracttype]
#[derive(Clone)]
pub enum MembershipStorageKey {
    /// The address that may issue and revoke membership.
    Issuer,
    /// The grant an account has made, if any.
    Grant(Address),
    /// The number of members holding at least one token.
    MemberCount,
    /// Whether an account has ever held a token, used to count members once.
    Counted(Address),
    /// The ledger an account first became a member on. Time weighted voting
    /// reads this, so it is set once and cleared only when the account stops
    /// being a member entirely.
    MemberSince(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MembershipError {
    /// The contract has no issuer.
    IssuerNotSet = 1,
    /// The caller is not the issuer.
    NotIssuer = 2,
    /// Membership tokens are credentials and cannot be transferred.
    NotTransferable = 3,
    /// The account holds no membership tokens, so it has nothing to grant.
    NoVotingUnits = 4,
    /// A term of zero ledgers would lapse the moment it was made.
    TermCannotBeZero = 5,
    /// The term is longer than the contract allows.
    TermTooLong = 6,
    /// The account has no grant to renew or withdraw.
    NoGrant = 7,
    /// The grant has not expired, so it cannot be lapsed yet.
    GrantStillLive = 8,
    /// The account already grants its power to this delegate for this term.
    AlreadyDelegated = 9,
}

/// Emitted when an account grants its voting power, to itself or to another.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantMade {
    #[topic]
    pub account: Address,
    #[topic]
    pub delegatee: Address,
    pub units: u128,
    pub expires_at: u32,
}

/// Emitted when an account renews a grant before it lapses.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantRenewed {
    #[topic]
    pub account: Address,
    #[topic]
    pub delegatee: Address,
    pub expires_at: u32,
}

/// Emitted when an account withdraws its grant before the term is up.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantWithdrawn {
    #[topic]
    pub account: Address,
    #[topic]
    pub delegatee: Address,
    pub units: u128,
}

/// Emitted when an expired grant is swept, taking the power out of the live
/// total.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrantLapsed {
    #[topic]
    pub account: Address,
    #[topic]
    pub delegatee: Address,
    pub units: u128,
    pub expired_at: u32,
}

/// Emitted when a membership token is issued.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipIssued {
    #[topic]
    pub to: Address,
    pub token_id: u32,
}

/// Emitted when a membership token is revoked.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MembershipRevoked {
    #[topic]
    pub from: Address,
    pub token_id: u32,
}

/// Emitted when the right to issue membership changes hands, which is how a
/// community moves issuance under governance.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IssuerChanged {
    #[topic]
    pub old_issuer: Address,
    #[topic]
    pub new_issuer: Address,
}

#![cfg(test)]
extern crate std;

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, BytesN, Env, Error, InvokeError, String,
};

use crate::{FactoryError, RatifyFactory, RatifyFactoryClient, Settings, Wasms};

const START: u32 = 10_000;

fn err(code: FactoryError) -> Result<Error, InvokeError> {
    Ok(Error::from_contract_error(code as u32))
}

fn settings(e: &Env, founder: &Address, guardian: &Address, name: &str) -> Settings {
    Settings {
        name: String::from_str(e, name),
        symbol: String::from_str(e, "RIVER"),
        base_uri: String::from_str(e, "https://example.invalid/member/"),
        founder: founder.clone(),
        guardian: guardian.clone(),
        timelock_delay: 500,
        voting_delay: 20,
        voting_period: 200,
        proposal_threshold: 1,
        quorum_bps: 2_000,
        contested_margin_bps: 2_000,
    }
}

/// A factory whose wasm hashes are placeholders.
///
/// Enough for the register, the validation and the address arithmetic. The
/// tests that actually deploy a community need real wasm and live in
/// [`wired`], behind the `wasm-tests` feature.
fn stub_factory<'a>(e: &Env) -> RatifyFactoryClient<'a> {
    let wasms = Wasms {
        membership: BytesN::from_array(e, &[1u8; 32]),
        weight_rule: BytesN::from_array(e, &[2u8; 32]),
        governor: BytesN::from_array(e, &[3u8; 32]),
        timelock: BytesN::from_array(e, &[4u8; 32]),
        treasury: BytesN::from_array(e, &[5u8; 32]),
        registry: BytesN::from_array(e, &[6u8; 32]),
    };
    RatifyFactoryClient::new(e, &e.register(RatifyFactory, (wasms,)))
}

// ################## THE REGISTER ##################

#[test]
fn a_new_factory_has_an_empty_register() {
    let e = Env::default();
    let factory = stub_factory(&e);

    assert_eq!(factory.community_count(), 0);
    assert_eq!(factory.communities(&0, &10).len(), 0);
    assert_eq!(
        factory.wasms().membership,
        BytesN::from_array(&e, &[1u8; 32])
    );
}

#[test]
fn paging_off_the_end_of_the_register_returns_nothing() {
    let e = Env::default();
    let factory = stub_factory(&e);

    assert_eq!(factory.communities(&50, &10).len(), 0);
    assert_eq!(factory.communities(&0, &0).len(), 0);
}

#[test]
fn asking_for_a_community_that_is_not_there_fails_plainly() {
    let e = Env::default();
    let factory = stub_factory(&e);

    assert_eq!(factory.try_community(&0), Err(err(FactoryError::NotFound)));
    assert_eq!(factory.community_by_governor(&Address::generate(&e)), None);
}

// ################## ADDRESSES BEFORE DEPLOYMENT ##################

#[test]
fn a_founder_can_see_where_their_community_will_live() {
    let e = Env::default();
    let factory = stub_factory(&e);
    let salt = BytesN::from_array(&e, &[7u8; 32]);

    let first = factory.addresses_for(&salt);
    let again = factory.addresses_for(&salt);

    // The same salt always gives the same six addresses, which is what lets
    // the timelock be built already knowing its governor.
    assert_eq!(first, again);
}

#[test]
fn every_contract_in_a_community_gets_its_own_address() {
    let e = Env::default();
    let factory = stub_factory(&e);
    let (membership, weight_rule, governor, timelock, treasury, registry) =
        factory.addresses_for(&BytesN::from_array(&e, &[7u8; 32]));

    let all = [
        membership,
        weight_rule,
        governor,
        timelock,
        treasury,
        registry,
    ];
    for i in 0..all.len() {
        for j in (i + 1)..all.len() {
            assert_ne!(all[i], all[j], "two contracts would collide at one address");
        }
    }
}

#[test]
fn two_communities_do_not_collide() {
    let e = Env::default();
    let factory = stub_factory(&e);

    let first = factory.addresses_for(&BytesN::from_array(&e, &[7u8; 32]));
    let second = factory.addresses_for(&BytesN::from_array(&e, &[8u8; 32]));

    assert_ne!(first.0, second.0);
    assert_ne!(first.2, second.2);
}

// ################## SETTINGS THAT WOULD NOT WORK ##################

#[test]
fn a_community_cannot_be_deployed_without_a_delay() {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().set_sequence_number(START);
    let factory = stub_factory(&e);
    let founder = Address::generate(&e);
    let guardian = Address::generate(&e);

    let mut bad = settings(&e, &founder, &guardian, "No delay");
    bad.timelock_delay = 0;

    assert_eq!(
        factory.try_deploy(&BytesN::from_array(&e, &[1u8; 32]), &bad),
        Err(err(FactoryError::DelayCannotBeZero))
    );
    assert_eq!(factory.community_count(), 0);
}

#[test]
fn a_community_cannot_be_deployed_with_a_voting_period_of_nothing() {
    let e = Env::default();
    e.mock_all_auths();
    let factory = stub_factory(&e);
    let founder = Address::generate(&e);
    let guardian = Address::generate(&e);

    let mut bad = settings(&e, &founder, &guardian, "No voting");
    bad.voting_period = 0;

    assert_eq!(
        factory.try_deploy(&BytesN::from_array(&e, &[1u8; 32]), &bad),
        Err(err(FactoryError::VotingPeriodCannotBeZero))
    );
}

#[test]
fn a_community_cannot_be_deployed_with_an_impossible_quorum() {
    let e = Env::default();
    e.mock_all_auths();
    let factory = stub_factory(&e);
    let founder = Address::generate(&e);
    let guardian = Address::generate(&e);

    let mut none = settings(&e, &founder, &guardian, "No quorum");
    none.quorum_bps = 0;
    assert_eq!(
        factory.try_deploy(&BytesN::from_array(&e, &[1u8; 32]), &none),
        Err(err(FactoryError::InvalidQuorum))
    );

    let mut more_than_all = settings(&e, &founder, &guardian, "Too much");
    more_than_all.quorum_bps = 10_001;
    assert_eq!(
        factory.try_deploy(&BytesN::from_array(&e, &[2u8; 32]), &more_than_all),
        Err(err(FactoryError::InvalidQuorum))
    );
}

#[test]
fn deploying_needs_the_founder_s_authorisation() {
    let e = Env::default();
    let factory = stub_factory(&e);
    let founder = Address::generate(&e);
    let guardian = Address::generate(&e);

    // No mocked auth at all.
    let result = factory.try_deploy(
        &BytesN::from_array(&e, &[1u8; 32]),
        &settings(&e, &founder, &guardian, "Unsigned"),
    );
    assert!(result.is_err());
    assert_eq!(factory.community_count(), 0);
}

/// Deploying a whole community, which needs the contracts compiled to wasm
/// first.
///
///     stellar contract build
///     cargo test --features wasm-tests
///
/// `bash scripts/test.sh` does both.
#[cfg(feature = "wasm-tests")]
mod wired {
    use super::*;
    use soroban_sdk::{token, IntoVal, Symbol, Val, Vec};

    mod membership_wasm {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32v1-none/release/ratify_membership.wasm"
        );
    }
    mod weight_rule_wasm {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32v1-none/release/ratify_weight_rule.wasm"
        );
    }
    mod governor_wasm {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32v1-none/release/ratify_governor.wasm"
        );
    }
    mod timelock_wasm {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32v1-none/release/ratify_timelock.wasm"
        );
    }
    mod treasury_wasm {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32v1-none/release/ratify_treasury.wasm"
        );
    }
    mod registry_wasm {
        soroban_sdk::contractimport!(
            file = "../../target/wasm32v1-none/release/ratify_delegate_registry.wasm"
        );
    }

    /// A factory loaded with the real contracts.
    fn real_factory<'a>(e: &Env) -> RatifyFactoryClient<'a> {
        let deployer = e.deployer();
        let wasms = Wasms {
            membership: deployer.upload_contract_wasm(membership_wasm::WASM),
            weight_rule: deployer.upload_contract_wasm(weight_rule_wasm::WASM),
            governor: deployer.upload_contract_wasm(governor_wasm::WASM),
            timelock: deployer.upload_contract_wasm(timelock_wasm::WASM),
            treasury: deployer.upload_contract_wasm(treasury_wasm::WASM),
            registry: deployer.upload_contract_wasm(registry_wasm::WASM),
        };
        RatifyFactoryClient::new(e, &e.register(RatifyFactory, (wasms,)))
    }

    #[test]
    fn a_community_arrives_wired_and_in_the_register() {
        let e = Env::default();
        e.mock_all_auths();
        e.ledger().set_sequence_number(START);

        let factory = real_factory(&e);
        let founder = Address::generate(&e);
        let guardian = Address::generate(&e);
        let salt = BytesN::from_array(&e, &[9u8; 32]);

        let predicted = factory.addresses_for(&salt);
        let community = factory.deploy(
            &salt,
            &settings(&e, &founder, &guardian, "Riverside Commons"),
        );

        assert_eq!(community.membership, predicted.0);
        assert_eq!(community.governor, predicted.2);

        assert_eq!(factory.community_count(), 1);
        assert_eq!(factory.community(&0), community);
        assert_eq!(
            factory.community_by_governor(&community.governor),
            Some(community.clone())
        );
        assert_eq!(factory.communities(&0, &10).len(), 1);
        assert_eq!(community.deployed_at, START);
        assert_eq!(community.name, String::from_str(&e, "Riverside Commons"));

        let timelock = timelock_wasm::Client::new(&e, &community.timelock);
        assert_eq!(timelock.governor(), community.governor);
        assert_eq!(timelock.guardian(), guardian);
        assert_eq!(timelock.get_min_delay(), 500);

        let treasury = treasury_wasm::Client::new(&e, &community.treasury);
        assert_eq!(treasury.timelock(), community.timelock);

        let registry = registry_wasm::Client::new(&e, &community.registry);
        assert_eq!(registry.governor(), community.governor);
        assert_eq!(registry.weight_rule(), community.weight_rule);

        let rule = weight_rule_wasm::Client::new(&e, &community.weight_rule);
        assert_eq!(rule.membership(), community.membership);

        let membership = membership_wasm::Client::new(&e, &community.membership);
        assert_eq!(membership.issuer(), founder);
        assert_eq!(membership.name(), String::from_str(&e, "Riverside Commons"));

        let governor = governor_wasm::Client::new(&e, &community.governor);
        assert_eq!(governor.timelock(), community.timelock);
        assert_eq!(governor.registry(), Some(community.registry.clone()));
        assert_eq!(governor.quorum_bps(), 2_000);
    }

    #[test]
    fn one_factory_serves_many_communities() {
        let e = Env::default();
        e.mock_all_auths();
        e.ledger().set_sequence_number(START);

        let factory = real_factory(&e);
        let guardian = Address::generate(&e);

        let first = factory.deploy(
            &BytesN::from_array(&e, &[1u8; 32]),
            &settings(&e, &Address::generate(&e), &guardian, "Riverside Commons"),
        );
        let second = factory.deploy(
            &BytesN::from_array(&e, &[2u8; 32]),
            &settings(&e, &Address::generate(&e), &guardian, "Harbour Trust"),
        );

        assert_eq!(factory.community_count(), 2);
        assert_ne!(first.governor, second.governor);
        assert_ne!(first.treasury, second.treasury);

        let first_treasury = treasury_wasm::Client::new(&e, &first.treasury);
        assert_eq!(first_treasury.timelock(), first.timelock);
        assert_ne!(first_treasury.timelock(), second.timelock);
    }

    #[test]
    fn a_deployed_community_can_govern_itself_from_the_start() {
        let e = Env::default();
        e.mock_all_auths();
        e.ledger().set_sequence_number(START);

        let factory = real_factory(&e);
        let founder = Address::generate(&e);
        let guardian = Address::generate(&e);
        let c = factory.deploy(
            &BytesN::from_array(&e, &[3u8; 32]),
            &settings(&e, &founder, &guardian, "Riverside Commons"),
        );

        let membership = membership_wasm::Client::new(&e, &c.membership);
        let treasury = treasury_wasm::Client::new(&e, &c.treasury);
        let governor = governor_wasm::Client::new(&e, &c.governor);

        let alice = Address::generate(&e);
        membership.issue(&alice);
        membership.delegate_for(&alice, &alice, &100_000);

        let issuer = Address::generate(&e);
        let asset = e.register_stellar_asset_contract_v2(issuer).address();
        token::StellarAssetClient::new(&e, &asset).mint(&c.treasury, &1_000_000);
        treasury.set_policy(&asset, &10_000, &50_000, &100_000);

        let recipient = Address::generate(&e);
        let description = String::from_str(&e, "Pay the roofer.");
        let args: Vec<Val> = Vec::from_array(
            &e,
            [
                asset.clone().into_val(&e),
                recipient.clone().into_val(&e),
                4_000i128.into_val(&e),
                BytesN::from_array(&e, &[0u8; 32]).into_val(&e),
            ],
        );
        let targets = Vec::from_array(&e, [c.treasury.clone()]);
        let functions = Vec::from_array(&e, [Symbol::new(&e, "pay")]);
        let all_args = Vec::from_array(&e, [args]);
        let description_hash = e.crypto().keccak256(&description.to_bytes()).to_bytes();

        e.ledger().set_sequence_number(START + 1);
        governor.propose(&targets, &functions, &all_args, &description, &alice);

        e.ledger().set_sequence_number(START + 22);
        let id = governor.get_proposal_id(&targets, &functions, &all_args, &description_hash);
        governor.cast_vote(&id, &1, &String::from_str(&e, ""), &alice);

        e.ledger().set_sequence_number(START + 1 + 20 + 200 + 1);
        governor.queue(
            &targets,
            &functions,
            &all_args,
            &description_hash,
            &0,
            &alice,
        );

        e.ledger().set_sequence_number(e.ledger().sequence() + 500);
        governor.execute(&targets, &functions, &all_args, &description_hash, &alice);

        assert_eq!(
            token::TokenClient::new(&e, &asset).balance(&recipient),
            4_000
        );
        assert_eq!(treasury.outflow_count(), 1);
    }
}

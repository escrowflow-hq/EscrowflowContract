#![cfg(test)]
// Amounts below are grouped as `<whole>_<7-decimal-stroops>` (Stellar's
// fixed-point convention), which clippy's default digit-grouping lint
// doesn't recognize as consistent.
#![allow(clippy::inconsistent_digit_grouping)]

use soroban_sdk::{testutils::Ledger, vec, Env, String};
use test_utils::{EscrowParties, TestToken};

use crate::{
    Dispute, DisputeOutcome, Error, EscrowContract, EscrowContractClient, EscrowStatus,
    MilestoneStatus,
};

const DAY: u64 = 24 * 60 * 60;
const DEFAULT_FEE_BPS: i128 = 300;
const BPS: i128 = 10_000;

fn fee_and_payout(amount: i128) -> (i128, i128) {
    let fee = amount * DEFAULT_FEE_BPS / BPS;
    (fee, amount - fee)
}

struct Setup<'a> {
    env: Env,
    parties: EscrowParties,
    token: TestToken<'a>,
    contract: EscrowContractClient<'a>,
}

fn setup(client_balance: i128) -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let parties = EscrowParties::new(&env);
    let token_admin = test_utils::generate_address(&env);
    let token = TestToken::new(&env, &token_admin);
    if client_balance > 0 {
        token.mint(&parties.client, client_balance);
    }

    let contract_id = env.register_contract(None, EscrowContract);
    let contract = EscrowContractClient::new(&env, &contract_id);
    contract.initialize(&parties.admin, &parties.arbitrator, &None);

    Setup {
        env,
        parties,
        token,
        contract,
    }
}

fn milestones(
    env: &Env,
) -> (
    soroban_sdk::Vec<String>,
    soroban_sdk::Vec<i128>,
    soroban_sdk::Vec<u64>,
) {
    let now = env.ledger().timestamp();
    let descriptions = vec![
        env,
        String::from_str(env, "design"),
        String::from_str(env, "implementation"),
        String::from_str(env, "delivery"),
    ];
    let amounts = vec![env, 200_0000000i128, 500_0000000i128, 300_0000000i128];
    let due_dates = vec![env, now + 10 * DAY, now + 20 * DAY, now + 30 * DAY];
    (descriptions, amounts, due_dates)
}

fn create_and_deposit(s: &Setup, total: i128) -> u64 {
    let (descriptions, amounts, due_dates) = milestones(&s.env);
    let id = s.contract.create_escrow(
        &s.parties.client,
        &s.parties.freelancer,
        &s.token.address,
        &descriptions,
        &amounts,
        &due_dates,
    );
    assert_eq!(s.contract.get_escrow(&id).total_amount, total);
    s.contract.deposit(&id, &s.parties.client);
    id
}

#[test]
fn test_initialize_twice_fails() {
    let s = setup(0);
    let result = s
        .contract
        .try_initialize(&s.parties.admin, &s.parties.arbitrator, &None);
    assert_eq!(result, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn test_initialize_rejects_invalid_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let parties = EscrowParties::new(&env);
    let contract_id = env.register_contract(None, EscrowContract);
    let contract = EscrowContractClient::new(&env, &contract_id);

    let result = contract.try_initialize(&parties.admin, &parties.arbitrator, &Some(10_001));
    assert_eq!(result, Err(Ok(Error::InvalidFeeBps)));
}

#[test]
fn test_full_lifecycle_completes_escrow() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);

    assert_eq!(s.token.balance(&s.contract.address), 1_000_0000000);
    assert_eq!(s.contract.get_escrow(&id).status, EscrowStatus::Active);

    for milestone_id in 0..3u32 {
        let amount = s.contract.get_milestone(&id, &milestone_id).amount;
        let (fee, payout) = fee_and_payout(amount);

        s.contract
            .submit_milestone(&id, &milestone_id, &s.parties.freelancer);
        assert_eq!(
            s.contract.get_milestone(&id, &milestone_id).status,
            MilestoneStatus::Submitted
        );

        let freelancer_before = s.token.balance(&s.parties.freelancer);
        let admin_before = s.token.balance(&s.parties.admin);

        s.contract
            .approve_milestone(&id, &milestone_id, &s.parties.client);

        assert_eq!(
            s.contract.get_milestone(&id, &milestone_id).status,
            MilestoneStatus::Released
        );
        assert_eq!(
            s.token.balance(&s.parties.freelancer),
            freelancer_before + payout
        );
        assert_eq!(s.token.balance(&s.parties.admin), admin_before + fee);
    }

    let escrow = s.contract.get_escrow(&id);
    assert_eq!(escrow.status, EscrowStatus::Completed);
    assert_eq!(escrow.released_amount, escrow.total_amount);
    assert_eq!(s.token.balance(&s.contract.address), 0);
}

#[test]
fn test_reject_and_resubmit_flow() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);

    s.contract.submit_milestone(&id, &0, &s.parties.freelancer);
    s.contract.reject_milestone(&id, &0, &s.parties.client);
    assert_eq!(
        s.contract.get_milestone(&id, &0).status,
        MilestoneStatus::Rejected
    );

    // Resubmitting a rejected milestone is allowed.
    s.contract.submit_milestone(&id, &0, &s.parties.freelancer);
    assert_eq!(
        s.contract.get_milestone(&id, &0).status,
        MilestoneStatus::Submitted
    );

    s.contract.approve_milestone(&id, &0, &s.parties.client);
    assert_eq!(
        s.contract.get_milestone(&id, &0).status,
        MilestoneStatus::Released
    );
}

#[test]
fn test_reject_requires_submitted_status() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);

    let result = s.contract.try_reject_milestone(&id, &0, &s.parties.client);
    assert_eq!(result, Err(Ok(Error::InvalidMilestoneStatus)));
}

#[test]
fn test_dispute_split_resolution() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);

    let amount = s.contract.get_milestone(&id, &0).amount; // 200_0000000
    s.contract.submit_milestone(&id, &0, &s.parties.freelancer);
    s.contract.open_dispute(&id, &0, &s.parties.client);

    assert_eq!(s.contract.get_escrow(&id).status, EscrowStatus::Disputed);
    assert_eq!(
        s.contract.get_milestone(&id, &0).status,
        MilestoneStatus::Disputed
    );

    let freelancer_before = s.token.balance(&s.parties.freelancer);
    let client_before = s.token.balance(&s.parties.client);
    let admin_before = s.token.balance(&s.parties.admin);

    // 60/40 split in the freelancer's favor.
    s.contract
        .resolve_dispute(&id, &0, &DisputeOutcome::Split, &Some(6_000));

    let freelancer_share = amount * 6_000 / BPS;
    let client_share = amount - freelancer_share;
    let (fee, payout) = fee_and_payout(freelancer_share);

    assert_eq!(
        s.token.balance(&s.parties.freelancer),
        freelancer_before + payout
    );
    assert_eq!(s.token.balance(&s.parties.admin), admin_before + fee);
    assert_eq!(
        s.token.balance(&s.parties.client),
        client_before + client_share
    );

    let escrow = s.contract.get_escrow(&id);
    assert_eq!(escrow.status, EscrowStatus::Active);
    assert_eq!(escrow.released_amount, amount);
    assert_eq!(
        s.contract.get_milestone(&id, &0).status,
        MilestoneStatus::Released
    );

    let dispute: Dispute = s.contract.get_dispute(&id, &0);
    assert!(dispute.resolved);
    assert_eq!(dispute.outcome, DisputeOutcome::Split);
}

#[test]
fn test_dispute_release_to_freelancer() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);
    let amount = s.contract.get_milestone(&id, &1).amount;

    s.contract.submit_milestone(&id, &1, &s.parties.freelancer);
    s.contract.open_dispute(&id, &1, &s.parties.freelancer);

    let (fee, payout) = fee_and_payout(amount);
    s.contract
        .resolve_dispute(&id, &1, &DisputeOutcome::ReleaseToFreelancer, &None);

    assert_eq!(s.token.balance(&s.parties.freelancer), payout);
    assert_eq!(s.token.balance(&s.parties.admin), fee);
}

#[test]
fn test_dispute_refund_to_client() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);
    let amount = s.contract.get_milestone(&id, &2).amount;

    s.contract.submit_milestone(&id, &2, &s.parties.freelancer);
    s.contract.open_dispute(&id, &2, &s.parties.client);

    let client_before = s.token.balance(&s.parties.client);
    s.contract
        .resolve_dispute(&id, &2, &DisputeOutcome::RefundToClient, &None);

    assert_eq!(s.token.balance(&s.parties.client), client_before + amount);
}

#[test]
fn test_resolve_dispute_twice_fails() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);
    s.contract.submit_milestone(&id, &0, &s.parties.freelancer);
    s.contract.open_dispute(&id, &0, &s.parties.client);
    s.contract
        .resolve_dispute(&id, &0, &DisputeOutcome::RefundToClient, &None);

    let result = s
        .contract
        .try_resolve_dispute(&id, &0, &DisputeOutcome::RefundToClient, &None);
    assert_eq!(result, Err(Ok(Error::InvalidStatus)));
}

#[test]
fn test_open_dispute_by_unrelated_party_fails() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);
    s.contract.submit_milestone(&id, &0, &s.parties.freelancer);

    let stranger = test_utils::generate_address(&s.env);
    let result = s.contract.try_open_dispute(&id, &0, &stranger);
    assert_eq!(result, Err(Ok(Error::UnknownParty)));
}

#[test]
fn test_open_dispute_requires_submitted_milestone() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);

    let result = s.contract.try_open_dispute(&id, &0, &s.parties.client);
    assert_eq!(result, Err(Ok(Error::InvalidMilestoneStatus)));
}

#[test]
fn test_submit_before_fully_funded_fails() {
    let s = setup(0);
    let (descriptions, amounts, due_dates) = milestones(&s.env);
    let id = s.contract.create_escrow(
        &s.parties.client,
        &s.parties.freelancer,
        &s.token.address,
        &descriptions,
        &amounts,
        &due_dates,
    );

    let result = s
        .contract
        .try_submit_milestone(&id, &0, &s.parties.freelancer);
    assert_eq!(result, Err(Ok(Error::NotFullyFunded)));
}

#[test]
fn test_deposit_twice_fails() {
    let s = setup(2_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);

    let result = s.contract.try_deposit(&id, &s.parties.client);
    assert_eq!(result, Err(Ok(Error::AlreadyDeposited)));
}

#[test]
fn test_submit_by_non_freelancer_fails() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);

    let result = s.contract.try_submit_milestone(&id, &0, &s.parties.client);
    assert_eq!(result, Err(Ok(Error::UnknownParty)));
}

#[test]
fn test_approve_by_non_client_fails() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);
    s.contract.submit_milestone(&id, &0, &s.parties.freelancer);

    let result = s
        .contract
        .try_approve_milestone(&id, &0, &s.parties.freelancer);
    assert_eq!(result, Err(Ok(Error::UnknownParty)));
}

#[test]
fn test_refund_by_admin() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);
    s.contract.submit_milestone(&id, &0, &s.parties.freelancer);
    s.contract.approve_milestone(&id, &0, &s.parties.client);

    let remaining =
        s.contract.get_escrow(&id).total_amount - s.contract.get_escrow(&id).released_amount;
    let client_before = s.token.balance(&s.parties.client);

    s.contract.refund(&id);

    assert_eq!(
        s.token.balance(&s.parties.client),
        client_before + remaining
    );
    assert_eq!(s.contract.get_escrow(&id).status, EscrowStatus::Refunded);
    assert_eq!(s.token.balance(&s.contract.address), 0);
}

#[test]
fn test_refund_after_completion_fails() {
    let s = setup(1_000_0000000);
    let id = create_and_deposit(&s, 1_000_0000000);
    for milestone_id in 0..3u32 {
        s.contract
            .submit_milestone(&id, &milestone_id, &s.parties.freelancer);
        s.contract
            .approve_milestone(&id, &milestone_id, &s.parties.client);
    }

    let result = s.contract.try_refund(&id);
    assert_eq!(result, Err(Ok(Error::InvalidStatus)));
}

#[test]
fn test_create_escrow_rejects_mismatched_milestones() {
    let s = setup(0);
    let (descriptions, _, due_dates) = milestones(&s.env);
    let amounts = vec![&s.env, 100i128];

    let result = s.contract.try_create_escrow(
        &s.parties.client,
        &s.parties.freelancer,
        &s.token.address,
        &descriptions,
        &amounts,
        &due_dates,
    );
    assert_eq!(result, Err(Ok(Error::InvalidMilestones)));
}

#[test]
fn test_create_escrow_rejects_past_due_date() {
    let s = setup(0);
    let (descriptions, amounts, _) = milestones(&s.env);
    let due_dates = vec![&s.env, 1u64, 2u64, 3u64];

    s.env.ledger().with_mut(|li| li.timestamp = 1_000);
    let result = s.contract.try_create_escrow(
        &s.parties.client,
        &s.parties.freelancer,
        &s.token.address,
        &descriptions,
        &amounts,
        &due_dates,
    );
    assert_eq!(result, Err(Ok(Error::InvalidDueDate)));
}

#[test]
fn test_get_escrow_not_found() {
    let s = setup(0);
    let result = s.contract.try_get_escrow(&42);
    assert_eq!(result, Err(Ok(Error::EscrowNotFound)));
}

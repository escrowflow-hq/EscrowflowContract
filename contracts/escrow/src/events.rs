use soroban_sdk::{symbol_short, Address, Env};

use crate::types::DisputeOutcome;

pub fn escrow_created(
    env: &Env,
    id: u64,
    client: &Address,
    freelancer: &Address,
    total_amount: i128,
) {
    env.events().publish(
        (symbol_short!("created"), id),
        (client.clone(), freelancer.clone(), total_amount),
    );
}

pub fn funds_deposited(env: &Env, id: u64, from: &Address, amount: i128) {
    env.events()
        .publish((symbol_short!("deposited"), id), (from.clone(), amount));
}

pub fn milestone_submitted(env: &Env, id: u64, milestone_id: u32) {
    env.events()
        .publish((symbol_short!("submitted"), id, milestone_id), ());
}

pub fn milestone_approved(env: &Env, id: u64, milestone_id: u32) {
    env.events()
        .publish((symbol_short!("approved"), id, milestone_id), ());
}

pub fn milestone_rejected(env: &Env, id: u64, milestone_id: u32) {
    env.events()
        .publish((symbol_short!("rejected"), id, milestone_id), ());
}

pub fn funds_released(
    env: &Env,
    id: u64,
    milestone_id: u32,
    to: &Address,
    amount: i128,
    fee: i128,
) {
    env.events().publish(
        (symbol_short!("released"), id, milestone_id),
        (to.clone(), amount, fee),
    );
}

pub fn dispute_opened(env: &Env, id: u64, milestone_id: u32, initiator: &Address) {
    env.events().publish(
        (symbol_short!("disputed"), id, milestone_id),
        initiator.clone(),
    );
}

pub fn dispute_resolved(env: &Env, id: u64, milestone_id: u32, outcome: &DisputeOutcome) {
    env.events().publish(
        (symbol_short!("resolved"), id, milestone_id),
        outcome.clone(),
    );
}

pub fn escrow_refunded(env: &Env, id: u64, amount: i128) {
    env.events()
        .publish((symbol_short!("refunded"), id), amount);
}

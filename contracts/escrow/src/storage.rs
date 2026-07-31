use soroban_sdk::{contracttype, Address, Env};

use crate::errors::Error;
use crate::types::{Dispute, Escrow, Milestone};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Platform admin: receives fees, may force-refund an escrow.
    Admin,
    /// Platform arbitrator: the sole signer able to resolve disputes.
    Arbitrator,
    /// Platform fee, in basis points (1/100th of a percent), taken on release.
    PlatformFeeBps,
    /// Monotonically increasing counter used to hand out escrow ids.
    EscrowCounter,
    Escrow(u64),
    Milestone(u64, u32),
    Dispute(u64, u32),
}

/// Ledgers are ~5s apart; these bump windows are expressed in that unit.
const LEDGERS_PER_DAY: u32 = 17_280;

const INSTANCE_BUMP_AMOUNT: u32 = 30 * LEDGERS_PER_DAY;
const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - LEDGERS_PER_DAY;

const PERSISTENT_BUMP_AMOUNT: u32 = 90 * LEDGERS_PER_DAY;
const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - LEDGERS_PER_DAY;

/// Keeps the contract instance (admin/arbitrator/fee/counter) alive.
pub fn bump_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Admin)
}

pub fn set_config(env: &Env, admin: &Address, arbitrator: &Address, fee_bps: u32) {
    env.storage().instance().set(&DataKey::Admin, admin);
    env.storage()
        .instance()
        .set(&DataKey::Arbitrator, arbitrator);
    env.storage()
        .instance()
        .set(&DataKey::PlatformFeeBps, &fee_bps);
}

pub fn get_admin(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(Error::NotInitialized)
}

pub fn get_arbitrator(env: &Env) -> Result<Address, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Arbitrator)
        .ok_or(Error::NotInitialized)
}

pub fn get_fee_bps(env: &Env) -> Result<u32, Error> {
    env.storage()
        .instance()
        .get(&DataKey::PlatformFeeBps)
        .ok_or(Error::NotInitialized)
}

/// Allocates and returns the next escrow id, persisting the updated counter.
pub fn next_escrow_id(env: &Env) -> u64 {
    let key = DataKey::EscrowCounter;
    let id: u64 = env.storage().instance().get(&key).unwrap_or(0);
    env.storage().instance().set(&key, &(id + 1));
    id
}

pub fn get_escrow(env: &Env, id: u64) -> Option<Escrow> {
    env.storage().persistent().get(&DataKey::Escrow(id))
}

pub fn set_escrow(env: &Env, escrow: &Escrow) {
    let key = DataKey::Escrow(escrow.id);
    env.storage().persistent().set(&key, escrow);
    env.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
}

pub fn get_milestone(env: &Env, escrow_id: u64, milestone_id: u32) -> Option<Milestone> {
    env.storage()
        .persistent()
        .get(&DataKey::Milestone(escrow_id, milestone_id))
}

pub fn set_milestone(env: &Env, escrow_id: u64, milestone: &Milestone) {
    let key = DataKey::Milestone(escrow_id, milestone.id);
    env.storage().persistent().set(&key, milestone);
    env.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
}

pub fn get_dispute(env: &Env, escrow_id: u64, milestone_id: u32) -> Option<Dispute> {
    env.storage()
        .persistent()
        .get(&DataKey::Dispute(escrow_id, milestone_id))
}

pub fn set_dispute(env: &Env, dispute: &Dispute) {
    let key = DataKey::Dispute(dispute.escrow_id, dispute.milestone_id);
    env.storage().persistent().set(&key, dispute);
    env.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
}

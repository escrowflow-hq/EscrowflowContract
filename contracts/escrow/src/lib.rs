#![no_std]

//! EscrowFlow — a decentralized escrow contract for freelance payments in
//! USDC (or any Stellar Asset Contract token) on Soroban.
//!
//! A client creates an escrow for a freelancer, broken into one or more
//! milestones. The freelancer submits each milestone as work is completed;
//! the client either approves it (releasing the payout, minus a platform
//! fee, to the freelancer) or rejects it (returning it to the freelancer
//! for rework). Either party can open a dispute on a submitted milestone,
//! which freezes the escrow until the platform's arbitrator resolves it —
//! releasing to the freelancer, refunding the client, or splitting the
//! amount between them. The platform admin can force-refund an escrow's
//! unreleased balance to the client at any time.

// The `#[contracttype]` macro derives `arbitrary::Arbitrary` whenever
// soroban-sdk's `testutils` feature is active anywhere in this build graph
// (e.g. via a dev-dependency during `cargo test`), and that generated impl
// references `std` unconditionally.
//
// IMPORTANT: don't make this unconditional — on a real (non-testutils)
// build, linking in the real `std` crate for wasm32-unknown-unknown
// conflicts with soroban-sdk's own `panic_impl` lang item. And don't run
// `cargo check`/`clippy --all-targets` (or `--tests` together with the
// plain lib) for this crate: Cargo's dev-dependency feature unification
// activates soroban-sdk's `testutils` for the plain lib target too in
// that case, tripping the same Arbitrary-derives-need-std issue with no
// corresponding `cfg(test)`/`cfg(feature = "testutils")` to catch it.
// Check/lint the lib and tests as separate invocations instead (see the
// Makefile's `lint` target).
#[cfg(any(test, feature = "testutils"))]
extern crate std;

mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod test;

pub use errors::Error;
pub use types::{Dispute, DisputeOutcome, Escrow, EscrowStatus, Milestone, MilestoneStatus};

use soroban_sdk::{contract, contractimpl, token, Address, Env, String, Vec};

const DEFAULT_FEE_BPS: u32 = 300; // 3%
const BPS_DENOMINATOR: i128 = 10_000;

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    /// One-time platform setup. Authorized by `admin`. `platform_fee_bps`
    /// defaults to 300 (3%) when omitted; must be between 0 and 10000.
    pub fn initialize(
        env: Env,
        admin: Address,
        arbitrator: Address,
        platform_fee_bps: Option<u32>,
    ) -> Result<(), Error> {
        admin.require_auth();

        if storage::is_initialized(&env) {
            return Err(Error::AlreadyInitialized);
        }
        let fee_bps = platform_fee_bps.unwrap_or(DEFAULT_FEE_BPS);
        if fee_bps > BPS_DENOMINATOR as u32 {
            return Err(Error::InvalidFeeBps);
        }

        storage::set_config(&env, &admin, &arbitrator, fee_bps);
        storage::bump_instance(&env);
        Ok(())
    }

    /// Creates a new escrow agreement with one or more milestones.
    /// Authorized by the client. Does not move any funds — call `deposit`
    /// afterwards to fund it.
    pub fn create_escrow(
        env: Env,
        client: Address,
        freelancer: Address,
        token: Address,
        milestone_descriptions: Vec<String>,
        milestone_amounts: Vec<i128>,
        milestone_due_dates: Vec<u64>,
    ) -> Result<u64, Error> {
        client.require_auth();

        if !storage::is_initialized(&env) {
            return Err(Error::NotInitialized);
        }

        let count = milestone_amounts.len();
        if count == 0 || milestone_descriptions.len() != count || milestone_due_dates.len() != count
        {
            return Err(Error::InvalidMilestones);
        }

        let now = env.ledger().timestamp();
        let mut total_amount: i128 = 0;
        for i in 0..count {
            let amount = milestone_amounts.get_unchecked(i);
            if amount <= 0 {
                return Err(Error::InvalidAmount);
            }
            if milestone_due_dates.get_unchecked(i) <= now {
                return Err(Error::InvalidDueDate);
            }
            total_amount = total_amount
                .checked_add(amount)
                .ok_or(Error::ArithmeticOverflow)?;
        }

        storage::bump_instance(&env);
        let id = storage::next_escrow_id(&env);

        for i in 0..count {
            let milestone = Milestone {
                id: i,
                description: milestone_descriptions.get_unchecked(i),
                amount: milestone_amounts.get_unchecked(i),
                due_date: milestone_due_dates.get_unchecked(i),
                status: MilestoneStatus::Pending,
            };
            storage::set_milestone(&env, id, &milestone);
        }

        let escrow = Escrow {
            id,
            client: client.clone(),
            freelancer: freelancer.clone(),
            token,
            total_amount,
            deposited_amount: 0,
            released_amount: 0,
            status: EscrowStatus::Active,
            milestone_count: count,
            created_at: now,
        };
        storage::set_escrow(&env, &escrow);

        events::escrow_created(&env, id, &client, &freelancer, total_amount);
        Ok(id)
    }

    /// Deposits the escrow's full `total_amount` from `from` into the
    /// contract. Authorized by `from`. Can only be called once per escrow.
    pub fn deposit(env: Env, escrow_id: u64, from: Address) -> Result<(), Error> {
        from.require_auth();

        let mut escrow = Self::load_escrow(&env, escrow_id)?;
        if escrow.status != EscrowStatus::Active {
            return Err(Error::InvalidStatus);
        }
        if escrow.deposited_amount != 0 {
            return Err(Error::AlreadyDeposited);
        }

        let token_client = token::Client::new(&env, &escrow.token);
        token_client.transfer(&from, &env.current_contract_address(), &escrow.total_amount);

        escrow.deposited_amount = escrow.total_amount;
        storage::set_escrow(&env, &escrow);

        events::funds_deposited(&env, escrow_id, &from, escrow.total_amount);
        Ok(())
    }

    /// Marks a milestone as done. Authorized by the escrow's freelancer.
    /// Requires the escrow to be fully funded and the milestone to be
    /// `Pending` or `Rejected`.
    pub fn submit_milestone(
        env: Env,
        escrow_id: u64,
        milestone_id: u32,
        freelancer: Address,
    ) -> Result<(), Error> {
        freelancer.require_auth();

        let escrow = Self::load_escrow(&env, escrow_id)?;
        if freelancer != escrow.freelancer {
            return Err(Error::UnknownParty);
        }
        if escrow.status != EscrowStatus::Active {
            return Err(Error::InvalidStatus);
        }
        if escrow.deposited_amount != escrow.total_amount {
            return Err(Error::NotFullyFunded);
        }

        let mut milestone = Self::load_milestone(&env, escrow_id, milestone_id)?;
        if milestone.status != MilestoneStatus::Pending
            && milestone.status != MilestoneStatus::Rejected
        {
            return Err(Error::InvalidMilestoneStatus);
        }

        milestone.status = MilestoneStatus::Submitted;
        storage::set_milestone(&env, escrow_id, &milestone);

        events::milestone_submitted(&env, escrow_id, milestone_id);
        Ok(())
    }

    /// Approves a submitted milestone, releasing its payout (minus the
    /// platform fee) to the freelancer and the fee to the admin.
    /// Authorized by the escrow's client. Completes the escrow once every
    /// milestone has been released.
    pub fn approve_milestone(
        env: Env,
        escrow_id: u64,
        milestone_id: u32,
        client: Address,
    ) -> Result<(), Error> {
        client.require_auth();

        let mut escrow = Self::load_escrow(&env, escrow_id)?;
        if client != escrow.client {
            return Err(Error::UnknownParty);
        }
        if escrow.status != EscrowStatus::Active {
            return Err(Error::InvalidStatus);
        }

        let mut milestone = Self::load_milestone(&env, escrow_id, milestone_id)?;
        if milestone.status != MilestoneStatus::Submitted {
            return Err(Error::InvalidMilestoneStatus);
        }

        let admin = storage::get_admin(&env)?;
        let fee_bps = storage::get_fee_bps(&env)?;
        let (payout, fee) = split_fee(milestone.amount, fee_bps)?;

        let token_client = token::Client::new(&env, &escrow.token);
        if payout > 0 {
            token_client.transfer(&env.current_contract_address(), &escrow.freelancer, &payout);
        }
        if fee > 0 {
            token_client.transfer(&env.current_contract_address(), &admin, &fee);
        }

        milestone.status = MilestoneStatus::Released;
        storage::set_milestone(&env, escrow_id, &milestone);

        escrow.released_amount = escrow
            .released_amount
            .checked_add(milestone.amount)
            .ok_or(Error::ArithmeticOverflow)?;
        if escrow.released_amount == escrow.total_amount {
            escrow.status = EscrowStatus::Completed;
        }
        storage::set_escrow(&env, &escrow);

        events::milestone_approved(&env, escrow_id, milestone_id);
        events::funds_released(
            &env,
            escrow_id,
            milestone_id,
            &escrow.freelancer,
            payout,
            fee,
        );
        Ok(())
    }

    /// Rejects a submitted milestone, sending it back to `Rejected` so the
    /// freelancer can resubmit. Authorized by the escrow's client.
    pub fn reject_milestone(
        env: Env,
        escrow_id: u64,
        milestone_id: u32,
        client: Address,
    ) -> Result<(), Error> {
        client.require_auth();

        let escrow = Self::load_escrow(&env, escrow_id)?;
        if client != escrow.client {
            return Err(Error::UnknownParty);
        }

        let mut milestone = Self::load_milestone(&env, escrow_id, milestone_id)?;
        if milestone.status != MilestoneStatus::Submitted {
            return Err(Error::InvalidMilestoneStatus);
        }

        milestone.status = MilestoneStatus::Rejected;
        storage::set_milestone(&env, escrow_id, &milestone);

        events::milestone_rejected(&env, escrow_id, milestone_id);
        Ok(())
    }

    /// Opens a dispute on a submitted milestone, freezing the whole
    /// escrow until the arbitrator resolves it. Authorized by, and
    /// restricted to, the escrow's client or freelancer.
    pub fn open_dispute(
        env: Env,
        escrow_id: u64,
        milestone_id: u32,
        initiator: Address,
    ) -> Result<(), Error> {
        initiator.require_auth();

        let mut escrow = Self::load_escrow(&env, escrow_id)?;
        if initiator != escrow.client && initiator != escrow.freelancer {
            return Err(Error::UnknownParty);
        }

        let mut milestone = Self::load_milestone(&env, escrow_id, milestone_id)?;
        if milestone.status != MilestoneStatus::Submitted {
            return Err(Error::InvalidMilestoneStatus);
        }

        let arbitrator = storage::get_arbitrator(&env)?;
        let dispute = Dispute {
            escrow_id,
            milestone_id,
            initiated_by: initiator.clone(),
            arbitrator,
            resolved: false,
            outcome: DisputeOutcome::ReleaseToFreelancer,
            created_at: env.ledger().timestamp(),
        };
        storage::set_dispute(&env, &dispute);

        milestone.status = MilestoneStatus::Disputed;
        storage::set_milestone(&env, escrow_id, &milestone);

        escrow.status = EscrowStatus::Disputed;
        storage::set_escrow(&env, &escrow);

        events::dispute_opened(&env, escrow_id, milestone_id, &initiator);
        Ok(())
    }

    /// Resolves an open dispute. Authorized by the platform's arbitrator.
    /// `split_bps` (freelancer's share, in basis points) is required only
    /// for `DisputeOutcome::Split` and ignored otherwise. The platform fee
    /// is taken from whatever share the freelancer receives.
    pub fn resolve_dispute(
        env: Env,
        escrow_id: u64,
        milestone_id: u32,
        outcome: DisputeOutcome,
        split_bps: Option<u32>,
    ) -> Result<(), Error> {
        let arbitrator = storage::get_arbitrator(&env)?;
        arbitrator.require_auth();

        let mut escrow = Self::load_escrow(&env, escrow_id)?;
        let mut milestone = Self::load_milestone(&env, escrow_id, milestone_id)?;
        let mut dispute =
            storage::get_dispute(&env, escrow_id, milestone_id).ok_or(Error::DisputeNotFound)?;

        if escrow.status != EscrowStatus::Disputed || milestone.status != MilestoneStatus::Disputed
        {
            return Err(Error::InvalidStatus);
        }
        if dispute.resolved {
            return Err(Error::DisputeAlreadyResolved);
        }

        let admin = storage::get_admin(&env)?;
        let fee_bps = storage::get_fee_bps(&env)?;
        let amount = milestone.amount;
        let token_client = token::Client::new(&env, &escrow.token);
        let contract_address = env.current_contract_address();

        match &outcome {
            DisputeOutcome::ReleaseToFreelancer => {
                let (payout, fee) = split_fee(amount, fee_bps)?;
                if payout > 0 {
                    token_client.transfer(&contract_address, &escrow.freelancer, &payout);
                }
                if fee > 0 {
                    token_client.transfer(&contract_address, &admin, &fee);
                }
            }
            DisputeOutcome::RefundToClient => {
                if amount > 0 {
                    token_client.transfer(&contract_address, &escrow.client, &amount);
                }
            }
            DisputeOutcome::Split => {
                let split_bps = split_bps.ok_or(Error::InvalidSplit)?;
                if split_bps as i128 > BPS_DENOMINATOR {
                    return Err(Error::InvalidSplit);
                }
                let freelancer_share = amount
                    .checked_mul(split_bps as i128)
                    .ok_or(Error::ArithmeticOverflow)?
                    / BPS_DENOMINATOR;
                let client_share = amount
                    .checked_sub(freelancer_share)
                    .ok_or(Error::ArithmeticOverflow)?;

                let (payout, fee) = split_fee(freelancer_share, fee_bps)?;
                if payout > 0 {
                    token_client.transfer(&contract_address, &escrow.freelancer, &payout);
                }
                if fee > 0 {
                    token_client.transfer(&contract_address, &admin, &fee);
                }
                if client_share > 0 {
                    token_client.transfer(&contract_address, &escrow.client, &client_share);
                }
            }
        }

        milestone.status = MilestoneStatus::Released;
        storage::set_milestone(&env, escrow_id, &milestone);

        escrow.released_amount = escrow
            .released_amount
            .checked_add(amount)
            .ok_or(Error::ArithmeticOverflow)?;
        escrow.status = if escrow.released_amount == escrow.total_amount {
            EscrowStatus::Completed
        } else {
            EscrowStatus::Active
        };
        storage::set_escrow(&env, &escrow);

        dispute.resolved = true;
        dispute.outcome = outcome.clone();
        storage::set_dispute(&env, &dispute);

        events::dispute_resolved(&env, escrow_id, milestone_id, &outcome);
        Ok(())
    }

    /// Force-refunds the escrow's unreleased balance to the client.
    /// Authorized by the platform admin.
    pub fn refund(env: Env, escrow_id: u64) -> Result<(), Error> {
        let admin = storage::get_admin(&env)?;
        admin.require_auth();

        let mut escrow = Self::load_escrow(&env, escrow_id)?;
        if escrow.status != EscrowStatus::Active && escrow.status != EscrowStatus::Disputed {
            return Err(Error::InvalidStatus);
        }

        let remaining = escrow
            .deposited_amount
            .checked_sub(escrow.released_amount)
            .ok_or(Error::ArithmeticOverflow)?;
        if remaining > 0 {
            let token_client = token::Client::new(&env, &escrow.token);
            token_client.transfer(&env.current_contract_address(), &escrow.client, &remaining);
        }

        escrow.released_amount = escrow.deposited_amount;
        escrow.status = EscrowStatus::Refunded;
        storage::set_escrow(&env, &escrow);

        events::escrow_refunded(&env, escrow_id, remaining);
        Ok(())
    }

    /// Read-only lookup of an escrow's current state.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Result<Escrow, Error> {
        Self::load_escrow(&env, escrow_id)
    }

    /// Read-only lookup of a milestone's current state.
    pub fn get_milestone(env: Env, escrow_id: u64, milestone_id: u32) -> Result<Milestone, Error> {
        Self::load_milestone(&env, escrow_id, milestone_id)
    }

    /// Read-only lookup of a dispute, if one has been opened for this
    /// escrow/milestone pair.
    pub fn get_dispute(env: Env, escrow_id: u64, milestone_id: u32) -> Result<Dispute, Error> {
        storage::get_dispute(&env, escrow_id, milestone_id).ok_or(Error::DisputeNotFound)
    }

    fn load_escrow(env: &Env, escrow_id: u64) -> Result<Escrow, Error> {
        storage::get_escrow(env, escrow_id).ok_or(Error::EscrowNotFound)
    }

    fn load_milestone(env: &Env, escrow_id: u64, milestone_id: u32) -> Result<Milestone, Error> {
        storage::get_milestone(env, escrow_id, milestone_id).ok_or(Error::MilestoneNotFound)
    }
}

/// Splits `amount` into `(payout, fee)` given `fee_bps` basis points,
/// rounding the fee down.
fn split_fee(amount: i128, fee_bps: u32) -> Result<(i128, i128), Error> {
    let fee = amount
        .checked_mul(fee_bps as i128)
        .ok_or(Error::ArithmeticOverflow)?
        / BPS_DENOMINATOR;
    let payout = amount.checked_sub(fee).ok_or(Error::ArithmeticOverflow)?;
    Ok((payout, fee))
}

use soroban_sdk::{contracttype, Address, String};

/// Lifecycle state of an escrow agreement.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscrowStatus {
    /// Created (and possibly deposited); milestones can be worked and released.
    Active,
    /// Every milestone has been released; nothing left to distribute.
    Completed,
    /// A milestone dispute is open; approvals/submissions are frozen.
    Disputed,
    /// Cancelled before completion.
    Cancelled,
    /// Admin refunded the unreleased balance to the client.
    Refunded,
}

/// Status of a single milestone within an escrow.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MilestoneStatus {
    /// Not yet submitted by the freelancer.
    Pending,
    /// Freelancer has marked the work done; awaiting client review.
    Submitted,
    /// Client approved the submission (transient, immediately followed by release).
    Approved,
    /// Funds for this milestone have left the escrow.
    Released,
    /// Client rejected the submission; freelancer may resubmit.
    Rejected,
    /// A dispute is open on this milestone.
    Disputed,
}

/// A unit of work with its own payout amount and due date, submitted by
/// the freelancer and approved by the client independently of other
/// milestones in the same escrow.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Milestone {
    pub id: u32,
    pub description: String,
    pub amount: i128,
    pub due_date: u64,
    pub status: MilestoneStatus,
}

/// A single escrow agreement between a client and a freelancer, held in
/// `token` (e.g. USDC) and released milestone-by-milestone. Milestones
/// themselves are stored separately, keyed by `(escrow_id, milestone_id)`.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escrow {
    pub id: u64,
    pub client: Address,
    pub freelancer: Address,
    pub token: Address,
    pub total_amount: i128,
    pub deposited_amount: i128,
    pub released_amount: i128,
    pub status: EscrowStatus,
    pub milestone_count: u32,
    pub created_at: u64,
}

/// Resolution a dispute can settle on.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeOutcome {
    ReleaseToFreelancer,
    RefundToClient,
    Split,
}

/// A dispute opened against a specific `Submitted` milestone, resolved by
/// the platform's arbitrator.
///
/// `outcome` is only meaningful once `resolved` is `true` (it defaults to
/// `ReleaseToFreelancer` as an inert placeholder before then). This is
/// used in place of `Option<DisputeOutcome>` because soroban-sdk's
/// `contracttype` derive only generates a fallible `ScVal` conversion for
/// custom enums, while `Option<T>`'s conversion (needed by the SDK's
/// testutils/arbitrary machinery) requires an infallible one.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dispute {
    pub escrow_id: u64,
    pub milestone_id: u32,
    pub initiated_by: Address,
    pub arbitrator: Address,
    pub resolved: bool,
    pub outcome: DisputeOutcome,
    pub created_at: u64,
}

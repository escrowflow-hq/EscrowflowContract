use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// `initialize` has already been called on this contract instance.
    AlreadyInitialized = 1,
    /// `initialize` has not been called yet.
    NotInitialized = 2,
    /// No escrow exists for the given id.
    EscrowNotFound = 3,
    /// No milestone exists at the given index for this escrow.
    MilestoneNotFound = 4,
    /// No dispute exists for the given escrow/milestone.
    DisputeNotFound = 5,
    /// Escrow is not in the required status for this action.
    InvalidStatus = 6,
    /// Milestone is not in the required status for this action.
    InvalidMilestoneStatus = 7,
    /// An amount supplied was zero or negative.
    InvalidAmount = 8,
    /// Milestone list was empty, or descriptions/amounts/due_dates length mismatched.
    InvalidMilestones = 9,
    /// `platform_fee_bps` must be between 0 and 10000 (basis points).
    InvalidFeeBps = 10,
    /// The escrow has not yet been fully deposited.
    NotFullyFunded = 11,
    /// The escrow has already received its deposit.
    AlreadyDeposited = 12,
    /// The caller is not one of the parties (client/freelancer) tied to this escrow.
    UnknownParty = 13,
    /// An arithmetic operation would have overflowed.
    ArithmeticOverflow = 14,
    /// `split_bps` must be between 0 and 10000 (basis points).
    InvalidSplit = 15,
    /// The dispute has already been resolved.
    DisputeAlreadyResolved = 16,
    /// `due_date` must be strictly in the future at creation time.
    InvalidDueDate = 17,
}

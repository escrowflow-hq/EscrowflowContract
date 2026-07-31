#![no_std]

//! Shared test helpers for EscrowFlow contracts.
//!
//! Kept in its own crate so contract test modules can spin up a mock
//! USDC-style token (a Stellar Asset Contract) and funded test accounts
//! without duplicating boilerplate in every `test.rs`.

use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

/// A test token (Stellar Asset Contract) plus its admin client and a
/// plain transfer/balance client, standing in for USDC in tests.
pub struct TestToken<'a> {
    pub address: Address,
    pub client: TokenClient<'a>,
    pub admin_client: StellarAssetClient<'a>,
}

impl<'a> TestToken<'a> {
    /// Deploys a new Stellar Asset Contract with `admin` as its issuer/admin.
    pub fn new(env: &Env, admin: &Address) -> Self {
        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        let address = sac.address();
        Self {
            client: TokenClient::new(env, &address),
            admin_client: StellarAssetClient::new(env, &address),
            address,
        }
    }

    /// Mints `amount` of the token to `to`.
    pub fn mint(&self, to: &Address, amount: i128) {
        self.admin_client.mint(to, &amount);
    }

    pub fn balance(&self, of: &Address) -> i128 {
        self.client.balance(of)
    }
}

/// Generates a fresh, funded-with-nothing test address.
pub fn generate_address(env: &Env) -> Address {
    Address::generate(env)
}

/// Convenience bundle of the parties involved in a typical escrow test: a
/// client (payer), a freelancer (payee), a platform admin, and an
/// arbitrator (dispute referee).
pub struct EscrowParties {
    pub client: Address,
    pub freelancer: Address,
    pub admin: Address,
    pub arbitrator: Address,
}

impl EscrowParties {
    pub fn new(env: &Env) -> Self {
        Self {
            client: Address::generate(env),
            freelancer: Address::generate(env),
            admin: Address::generate(env),
            arbitrator: Address::generate(env),
        }
    }
}

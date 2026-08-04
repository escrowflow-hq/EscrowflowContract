# Testnet Deployment

EscrowFlow deployed and smoke-tested end-to-end on Stellar **testnet** on 2026-08-04.

## Network

| | |
|---|---|
| Network | Stellar Testnet (`Test SDF Network ; September 2015`) |
| RPC URL | https://soroban-testnet.stellar.org |
| Horizon / Friendbot | https://friendbot.stellar.org |

## Contract IDs

| Contract | ID |
|---|---|
| **EscrowFlow (escrow)** | `CBD4P6Q7YVE6JXGVLZFJSBYQSTX33MQM2ICUQ4EDOEC4ELOZICTDFKJN` |
| **Test USDC (TUSDC, Stellar Asset Contract)** | `CBNBYRHQPPONRDLW6ZKPNYS3RPRLKFH2AXDFQM7CMZQF65YIO4YUCJQZ` |

- Wasm hash: `166d0d5dd0f3b3c6b6643cf35ca34efebc401065e6519a86a0fe9b3f68b1db77`
- TUSDC is a classic Stellar asset (code `TUSDC`, issuer = deployer account below) wrapped as a Soroban Asset Contract via `stellar contract asset deploy`. It behaves exactly like a real SAC token (e.g. USDC) for integration purposes, including the requirement that holders establish a classic trustline before they can hold a balance.

## Roles / public keys

No secret keys are stored in this repo. Each identity's secret key was printed once to the terminal at generation time — see the operator who ran the deployment for safekeeping (e.g. a secrets manager), or regenerate fresh identities for any real integration.

| Role | Public key |
|---|---|
| Deployer / TUSDC issuer | `GBNF554UOLQPPVULESLZM7K7HDJLW5LVRDN567JMIF733MAM5SZJ6XZX` |
| Admin (`initialize` admin, fee recipient) | `GBHLPUL4LFD24N2EKOSH5UHM7RPR3F3DE5JDUXODVMMUYO4WAAJ3H6J5` |
| Arbitrator (`initialize` arbitrator) | `GAXFOYRPFHG2U3R7XZ5HA3BBRBC7254LPASRE5QG5Q2TDWC7AT2EAGN6` |
| Client (smoke test only) | `GCHXGZW5QWV7Q26EE6B3GNMIMFQ5APTVGKIGRAPE4UTJ3LJBGYHFVCCS` |
| Freelancer (smoke test only) | `GBU7RZVV7TIV7FYFZI5H372FE53EFAOT527HF3SXCVEPAN3EGOTM4RX5` |

The contract was initialized with the default platform fee: **300 bps (3%)**.

## Integrating

- Read/simulate calls (`get_escrow`, `get_milestone`, `get_dispute`, token `balance`) need no signature.
- State-changing calls need a `require_auth()` signature from the relevant party (client, freelancer, admin, or arbitrator) per the table in the [README](README.md#public-functions).
- Any account that will **hold** TUSDC (client depositing, freelancer/admin receiving payouts) must first establish a classic trustline to `TUSDC:GBNF554UOLQPPVULESLZM7K7HDJLW5LVRDN567JMIF733MAM5SZJ6XZX` (`stellar tx new change-trust --line "TUSDC:GBNF554UOLQPPVULESLZM7K7HDJLW5LVRDN567JMIF733MAM5SZJ6XZX"`), same as for any classic/SAC asset such as real USDC.
- Amounts are `i128` in stroops, 7 decimal places (`1_0000000` = 1.0 TUSDC).

## Smoke test (proof of a working deployment)

Full lifecycle exercised on testnet: escrow created with 2 milestones (100 + 50 TUSDC), funded, both milestones submitted and approved, freelancer paid out net of the 3% platform fee.

| Step | Result | Tx hash |
|---|---|---|
| Upload contract wasm | success | [`06a99ec977cadc5e2b4e3bca2bdeda30b507fc45aae854c2e12627e17f761fa5`](https://stellar.expert/explorer/testnet/tx/06a99ec977cadc5e2b4e3bca2bdeda30b507fc45aae854c2e12627e17f761fa5) |
| Deploy contract | success | [`fa572cc4797ba30bc8ef8a39a1d68ec1b757ff64fee706eb923f805ffd746ef8`](https://stellar.expert/explorer/testnet/tx/fa572cc4797ba30bc8ef8a39a1d68ec1b757ff64fee706eb923f805ffd746ef8) |
| Deploy TUSDC SAC | success | [`3eac8fd22c5897b5eb306f2fb43c6f6fff0e09c0290696cccc2d37905ea8d907`](https://stellar.expert/explorer/testnet/tx/3eac8fd22c5897b5eb306f2fb43c6f6fff0e09c0290696cccc2d37905ea8d907) |
| `initialize(admin, arbitrator, None)` | success (fee = 300 bps) | [`0a18718b9aeb606d0527b75103a8bd3c85e5e894db2cd58ce7fe4b40a82bf16c`](https://stellar.expert/explorer/testnet/tx/0a18718b9aeb606d0527b75103a8bd3c85e5e894db2cd58ce7fe4b40a82bf16c) |
| Trustline: client → TUSDC | success | [`3e206c2a800b812ed7bf0e319834f02aa879459535ba9354121a2b074f6ca27b`](https://stellar.expert/explorer/testnet/tx/3e206c2a800b812ed7bf0e319834f02aa879459535ba9354121a2b074f6ca27b) |
| Trustline: freelancer → TUSDC | success | [`50aa2d24f7d952b706f36fa55a89581146bccbe738806ec579af54522231c9d5`](https://stellar.expert/explorer/testnet/tx/50aa2d24f7d952b706f36fa55a89581146bccbe738806ec579af54522231c9d5) |
| Trustline: admin → TUSDC | success | [`caadd7f279e1f32040a7e25d440a6515670b3e7b44d9c8344e45ea30e7754c71`](https://stellar.expert/explorer/testnet/tx/caadd7f279e1f32040a7e25d440a6515670b3e7b44d9c8344e45ea30e7754c71) |
| Mint 1,000 TUSDC to client | success | [`03bb7030492fd123f4d5eeb6907ae6f07b5343e9b3723de027f501c143f0a34b`](https://stellar.expert/explorer/testnet/tx/03bb7030492fd123f4d5eeb6907ae6f07b5343e9b3723de027f501c143f0a34b) |
| `create_escrow` (escrow id `0`, milestones: 100 + 50 TUSDC) | success | [`9af689a7c70ca4b7d5a5afe5a51e8bcfb12874505fba829f6c91fcc050f671a8`](https://stellar.expert/explorer/testnet/tx/9af689a7c70ca4b7d5a5afe5a51e8bcfb12874505fba829f6c91fcc050f671a8) |
| `deposit(0, client)` — 150 TUSDC | success | [`1296f0668ee168ea594505feb01d27a1d54be887a386ba1a4d604cd80d111a1f`](https://stellar.expert/explorer/testnet/tx/1296f0668ee168ea594505feb01d27a1d54be887a386ba1a4d604cd80d111a1f) |
| `submit_milestone(0, 0, freelancer)` | success | [`7655fc84acad4ef5f9c57ecec9e3afb6e30453795ef435602a28a8c178c63629`](https://stellar.expert/explorer/testnet/tx/7655fc84acad4ef5f9c57ecec9e3afb6e30453795ef435602a28a8c178c63629) |
| `approve_milestone(0, 0, client)` — pays 97 TUSDC to freelancer, 3 TUSDC fee to admin | success | [`c2df430cfb8eb316948c5d2869c04d9ef772812e649088fbad3fc976fad29630`](https://stellar.expert/explorer/testnet/tx/c2df430cfb8eb316948c5d2869c04d9ef772812e649088fbad3fc976fad29630) |
| `submit_milestone(0, 1, freelancer)` | success | [`cf643ebc97393eacd31dc83094313fae4cefd720dd88ea31b1a28b070f8d7639`](https://stellar.expert/explorer/testnet/tx/cf643ebc97393eacd31dc83094313fae4cefd720dd88ea31b1a28b070f8d7639) |
| `approve_milestone(0, 1, client)` — pays 48.5 TUSDC to freelancer, 1.5 TUSDC fee to admin | success | [`d11db8434b23b9d36052460c07f63883d0f23f8caf0d75727740ca38f6f10d52`](https://stellar.expert/explorer/testnet/tx/d11db8434b23b9d36052460c07f63883d0f23f8caf0d75727740ca38f6f10d52) |

### Verified end state

- `get_escrow(0)`: `status = Completed`, `deposited_amount = released_amount = 1,500,000,000` (150.0 TUSDC).
- Freelancer TUSDC balance: `1,455,000,000` (145.5 TUSDC = 97% of 150, i.e. 100% minus the 3% fee on each of the two milestones).
- Admin TUSDC balance: `45,000,000` (4.5 TUSDC = 3% of 150).
- Client TUSDC balance: `8,500,000,000` (850 = 1,000 minted − 150 deposited).

Fee math checks out exactly against the 300 bps default: `100 → 97 / 3`, `50 → 48.5 / 1.5`.

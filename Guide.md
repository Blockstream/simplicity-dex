# Simplicity DEX — Developer Guide

This short guide helps contributors understand, build, test and extend the project. It focuses on practical commands and
the patterns used across crates (not exhaustive; follow Rust and crate docs for deeper dives).

## Project layout

- crates/dex-cli — command line client and UX helpers
- crates/dex-nostr-relay — relay logic, event parsing and storage
- crates/global-utils — other helpers

## Prerequisites

- Install Rust
- Create your nostr keypair (Can be generated here: https://start.nostr.net/)

## Quick start

1. Build:
    - cargo build -r
2. Run CLI (local dev):
    - `cargo build -r`
    - `mkdir -p ./demo`
    - `mv ./target/release/simplicity-dex ./demo/simplicity-dex`
    - `cp ./.simplicity-dex.example/.simplicity-dex.config.toml ./demo/.simplicity-dex.config.toml`
    - `echo SEED_HEX=ffff0123456789abcdef0123456789abcdef0123456789abcdef0123456789ab > ./demo/.env`
3. Insert your valid nostr keypair into `.simplicity-dex.config.toml`

## Commands example execution

Overall trading for dcd contracts can be split in two sides: taker and maker.

Maker and Taker responsible for taking such steps:

1) Maker initializes contract in Liquid;
2) Maker funds contract with collateral and settlement tokens. (by now for test **collateral** = LBTC-Testnet, **settlement** = minted token from scratch)
3) Taker funds contract with collateral tokens and takes contract parameters from already discovered maker event_id.
4) Maker now can make:
   * Early collateral termination
   * Early settlement termination
5) Taker now can make:
   * Early termination
6) After `settlement-height` both maker and taker can use settlement exit to receive their tokens (collateral or settlement) depending on the settlement token price, which is signed with oracle.

1. Create your own contract with your values. For example can be taken

* `taker-funding-start-time` 1764328373 (timestamp can be taken from https://www.epochconverter.com/)
* `taker-funding-end-time` 1764358373 (Block time when taker funding period ends)
* `contract-expiry-time` 1764359373 (Block time when contract expires)
* `early-termination-end-time` 1764359373 (Block time when early termination is no longer allowed)
* `settlement-height` 2169368 (Block height at which oracle price is attested)
* `principal-collateral-amount` 2000 (Base collateral amount)
* `incentive-basis-points` 1000 (Incentive in basis points (1 bp = 0.01%))
* `filler-per-principal-collateral` 100 (Filler token ratio)
* `strike-price` 25 (Oracle strike price for settlement)
* `settlement-asset-entropy` `0ffa97b7ee6fcaac30b0c04803726f13c5176af59596874a3a770cbfd2a8d183`  (Asset entropy (hex) for settlement)
* `oracle-pubkey` `757f7c05d2d8f92ab37b880710491222a0d22b66be83ae68ff75cc6cb15dd2eb` (`./simplicity-dex helpers address --account-index 5`)

Actual command in cli:
```bash
./simplicity-dex maker init
  --utxo-1 <FIRST_LBTC_UTXO>
  --utxo-2 <SECOND_LBTC_UTXO>
  --utxo-3 <THIRD_LBTC_UTXO>
  --taker-funding-start-time <TAKER_FUNDING_START_TIME>
  --taker-funding-end-time <TAKER_FUNDING_END_TIME>
  --contract-expiry-time <CONTRACT_EXPIRY_TIME>
  --early-termination-end-time <EARLY_TERMINATION_END_TIME>
  --settlement-height <SETTLEMENT_HEIGHT>
  --principal-collateral-amount <PRINCIPAL_COLLATERAL_AMOUNT>
  --incentive-basis-points <INCENTIVE_BASIS_POINTS>
  --filler-per-principal-collateral <FILLER_PER_PRINCIPAL_COLLATERAL>
  --strike-price <STRIKE_PRICE>
  --settlement-asset-entropy <SETTLEMENT_ASSET_ENTROPY>
  --oracle-pubkey <ORACLE_PUBLIC_KEY>
```

2. Maker fund cli command: 
```bash
./simplicity-dex maker fund
  --filler-utxo <FILLER_TOKEN_UTXO>
  --grant-coll-utxo <GRANTOR_COLLATERAL_TOKEN_UTXO>
  --grant-settl-utxo <GRANTOR_SETTLEMENT_TOKEN_UTXO>
  --settl-asset-utxo <SETTLEMENT_ASSET_UTXO>
  --fee-utxo <FEE_UTXO>
  --taproot-pubkey-gen <DCD_TAPROOT_PUBKEY_GEN>
```

3. Taker has to fund 

```bash
./simplicity-dex taker fund
  --filler-utxo <FILLER_TOKEN_UTXO>
  --collateral-utxo <COLLATERAL_TOKEN_UTXO>
  --collateral-amount-deposit <COLLATERAL_AMOUNT_TO_DEPOSIT>
  --maker-order-event-id <MAKER_ORDER_EVENT_ID>
```

4. Taker can wait for specific `settlement-height` and gracefully exit contract: 
```bash
./simplicity-dex taker settlement
  --filler-utxo <FILLER_TOKEN_UTXO>
  --asset-utxo <ASSET_UTXO>
  --fee-utxo <FEE_UTXO>
  --filler-to-burn <FILLER_AMOUNT_TO_BURN>
  --price-now <PRICE_AT_CURRENT_BLOCK_HEIGHT>
  --oracle-sign <ORACLE_SIGNATURE>
  --maker-order-event-id <MAKER_ORDER_EVENT_ID>
```

5. Maker can wait for specific `settlement-height` and gracefully exit contract: 
```bash
 ./simplicity-dex maker settlement
  --grant-collateral-utxo <GRANTOR_COLLATERAL_TOKEN_UTXO>
  --grant-settlement-utxo <GRANTOR_SETTLEMENT_TOKEN_UTXO>
  --asset-utxo <ASSET_UTXO>
  --fee-utxo <FEE_UTXO>
  --grantor-amount-burn <GRANTOR_AMOUNT_TO_BURN>
  --price-now <PRICE_AT_CURRENT_BLOCK_HEIGHT>
  --oracle-sign <ORACLE_SIGNATURE>
  --maker-order-event-id <MAKER_ORDER_EVENT_ID>
```

* Maker or Taker depending on the can use Merge(2/3/4) command to merge collateral tokens.
This is made exactly for combining outs into one to eliminate execution of contract with usage of little fragments
```bash
./simplicity-dex helpers merge-tokens4
  --token-utxo-1 <TOKEN_UTXO_1>
  --token-utxo-2 <TOKEN_UTXO_2>
  --token-utxo-3 <TOKEN_UTXO_3>
  --token-utxo-4 <TOKEN_UTXO_4>
  --fee-utxo <FEE_UTXO>
  --maker-order-event-id <MAKER_ORDER_EVENT_ID>
```

* For early collateral termination Maker can use command: 
```bash
./simplicity-dex maker termination-collateral
  --grantor-collateral-utxo <GRANTOR_COLLATERAL_TOKEN_UTXO>
  --collateral-utxo <COLLATERAL_TOKEN_UTXO>
  --fee-utxo <FEE_UTXO>
  --grantor-collateral-burn <GRANTOR_COLLATERAL_AMOUNT_TO_BURN>
  --maker-order-event-id <MAKER_ORDER_EVENT_ID> 
```

* For early settlement termination Maker can use command:
```bash
./simplicity-dex maker termination-settlement
  --settlement-asset-utxo <SETTLEMENT_ASSET_UTXO>
  --grantor-settlement-utxo <GRANTOR_SETTLEMENT_TOKEN_UTXO>
  --fee-utxo <FEE_UTXO>
  --grantor-settlement-amount-burn <GRANTOR_SETTLEMENT_AMOUNT_TO_BURN>
  --maker-order-event-id <MAKER_ORDER_EVENT_ID>
```

* For early termination Taker can use command:
```bash
./simplicity-dex taker termination-early
  --filler-utxo <FILLER_TOKEN_UTXO>
  --collateral-utxo <COLLATERAL_TOKEN_UTXO>
  --fee-utxo <FEE_UTXO>
  --filler-to-return <FILLER_TOKEN_AMOUNT_TO_RETURN>
  --maker-order-event-id <MAKER_ORDER_EVENT_ID
```
# options-relay

NOSTR relay library for Simplicity Options trading on Liquid Network.

## Features

- Stream and fetch option creation events
- Stream and fetch swap (atomic swap with change) events  
- Track action completions (exercise, claim, expiry)
- Event signature verification
- TaprootPubkeyGen validation on parse

## Oracle-Free Settlement

- Maker holds Option Token, Taker holds Grantor Token
- Maker exercises when profitable, Taker claims after expiry or exercise
- No price oracle needed - relies on natural incentives

## NOSTR Events

| Kind | Name | Purpose |
|------|------|---------|
| 9910 | OPTION_CREATED | Options contract funded |
| 9911 | SWAP_CREATED | Atomic swap offer |
| 9912 | ACTION_COMPLETED | Exercise, expire, claim, or cancel |

## Swap Contract

Token trading uses the Simplicity `swap_with_change` contract for atomic swaps with change support.

## To be Done

- [ ] Extended filters for event queries:
  - `since` / `until` - Filter by time range to find active (non-expired) contracts
  - `limit` - Pagination for large result sets
  - `authors` - Filter events by creator's public key ("show only my options/swaps")


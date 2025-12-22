# options-relay

NOSTR relay library for Simplicity Options trading on Liquid Network.

## Features

- Stream and fetch option creation events
- Stream and fetch swap (atomic swap with change) events  
- Track action completions (exercise, claim, expiry)
- Event signature verification
- TaprootPubkeyGen validation on parse

## To be Done

- [ ] Extended filters for event queries:
  - `since` / `until` - Filter by time range to find active (non-expired) contracts
  - `limit` - Pagination for large result sets
  - `authors` - Filter events by creator's public key ("show only my options/swaps")


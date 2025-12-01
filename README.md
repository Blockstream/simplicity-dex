# Simplicity DEX

A distributed exchange built on the NOSTR protocol, leveraging Simplicity smart contracts and the PACT (PACT for
Auditable Contract Transactions) messaging protocol.

## Overview

Simplicity DEX is a decentralized exchange that combines the power of Simplicity smart contracts with the distributed
messaging capabilities of NOSTR. By utilizing the PACT protocol, we enable secure, auditable, and transparent trading of
digital assets without relying on centralized intermediaries.

## Key Features

- **Decentralized Architecture**: Built on NOSTR for censorship-resistant, distributed messaging
- **Simplicity Smart Contracts**: Leveraging Bitcoin's Simplicity language for provably secure contract execution
- **PACT Protocol**: Standardized format for auditable contract transactions
- **Open Ecosystem**: Compatible with any NOSTR client for maximum interoperability
- **Maker Identity Registry**: On-chain reputation system for market makers

## DEX Messaging Protocol

The core of our DEX is the **PACT (PACT for Auditable Contract Transactions)** protocol, which defines the format of
trading offers. This protocol is fully adapted to be compatible with the NOSTR event structure.

### Offer Structure

A PACT offer is implemented as a standard NOSTR event with kind `30078` (non-standard, ephemeral event kind for DEX
offers). The event structure maps to PACT requirements as follows:

| NOSTR Field   | PACT Field    | Data Type             | Required | Description                                                                                                       |
|---------------|---------------|-----------------------|----------|-------------------------------------------------------------------------------------------------------------------|
| `id`          | Event ID      | string (64-char hex)  | Yes      | SHA-256 hash of canonical serialized event data (excluding `sig`). Serves as unique, content-addressed identifier |
| `pubkey`      | Maker Key     | string (64-char hex)  | Yes      | 32-byte x-only Schnorr public key of market maker. Must be registered in on-chain Maker Identity Registry         |
| `created_at`  | Timestamp     | integer               | Yes      | Unix timestamp (seconds) when offer was created                                                                   |
| `description` | Description   | string                | No       | Human-readable description of instrument and complex terms                                                        |
| `kind`        | Event Type    | integer               | Yes      | Event type identifier. Value `1` reserved for standard offers. Enables future protocol extensions                 |
| `tags`        | Metadata      | array of arrays       | Yes      | Structured machine-readable metadata for filtering and discovery                                                  |
| `content`     | Contract Code | string                | Yes      | Stringified JSON containing full Simplicity contract code                                                         |
| `sig`         | Signature     | string (128-char hex) | Yes      | 64-byte Schnorr signature proving authenticity and integrity                                                      |

### Tag Examples

The `tags` field contains structured metadata as key-value pairs:

```json
[
  [
    "asset_to_sell",
    "<liquid_asset_id>"
  ],
  [
    "asset_to_buy",
    "<liquid_asset_id>"
  ],
  [
    "price",
    "1000000",
    "sats_per_contract"
  ],
  [
    "expiry",
    "1735689600"
  ],
  [
    "compiler",
    "simplicity-v1.2.3",
    "deterministic_build_hash"
  ]
]
```

### Protocol Benefits

- **Interoperability**: Any NOSTR-compatible client can parse and validate offers
- **Transparency**: All offers are publicly auditable
- **Censorship Resistance**: Distributed messaging prevents single points of failure
- **Standardization**: Consistent format enables ecosystem growth
- **Extensibility**: Protocol designed for future enhancements

## Getting Started

### Basic Usage

1. **Create an Offer**: Generate a PACT-compliant NOSTR event with your trading parameters
2. **Broadcast**: Publish the offer to NOSTR relays
3. **Discovery**: Takers can filter and discover offers using tag-based queries
4. **Execution**: Complete trades through Simplicity contract execution

## Architecture

```text
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Maker Client  │     │  NOSTR Relays    │     │  Taker Client   │
│                 │<───>|                  │<───>│                 │
│ - Create Offers │     │ - Store Events   │     │ - Discover      │
│ - Sign Contracts│     │ - Relay Messages │     │ - Execute Trades│
└─────────────────┘     └──────────────────┘     └─────────────────┘
         │                       │                       │
         │              ┌──────────────────┐             │
         └─────────────>│ Liquid Network   │<────────────┘
                        │                  │
                        │ - Asset Registry │
                        │ - Contract Exec  │
                        │ - Settlement     │
                        └──────────────────┘
```

## Contributing

We welcome contributions to the Simplicity DEX project.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Links

- [Simplicity Language](https://github.com/ElementsProject/simplicity)
- [NOSTR Protocol](https://github.com/nostr-protocol/nostr)
- [Liquid Network](https://liquid.net/)

## Disclaimer

This software is experimental and should be used with caution. Always verify contract code and understand the risks
before trading.
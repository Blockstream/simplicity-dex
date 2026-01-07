# Simplicity DEX

Options trading on Liquid Network using Simplicity.

## Features

- Oracle-free options settlement via natural economic incentives
- NOSTR-based contract discovery and event broadcasting
- Swaps using Simplicity `swap_with_change` contract
- CLI client for creating, exercising, and settling options

## Crates

| Crate           | Description                                |
|-----------------|--------------------------------------------|
| `cli-client`    | Command-line interface for options trading |
| `options-relay` | NOSTR relay library for contract discovery |
| `coin-store`    | UTXO storage and query engine              |
| `signer`        | Transaction signing utilities              |

## Useful Resources

- [Simplicity Language](https://github.com/ElementsProject/simplicity)
- [NOSTR Protocol](https://github.com/nostr-protocol/nostr)
- [Liquid Network](https://liquid.net/)

## Disclaimer

This software is experimental and should be used with caution. Always verify contract code and understand the risks
before trading.
# sign-txs

Sign Bitcoin transactions using a bitcoind wallet running in a Docker container.

This utility reads unsigned (or partially signed) transactions from a JSON file, fetches the required prevout information using a local `bitcoin-cli`, and signs them using a wallet in a dockerized bitcoind instance.

## Installation

```sh
cargo install sign-txs
```

## Requirements

- `bitcoin-cli` available in PATH (for decoding transactions and fetching prevout info)
- Docker with a running bitcoind container that has a loaded wallet

## Usage

```sh
sign-txs [OPTIONS] [INPUT_FILE]
```

### Arguments

- `INPUT_FILE` - JSON file containing transactions (default: `txs.json`)

### Options

- `--bitcoind-container <ID>` - Docker container ID running bitcoind with the wallet (can also be set via `BITCOIND_CONTAINER` environment variable)

### Input Format

The input JSON file should contain an array of transaction objects:

```json
[
  { "bitcoin": "<raw_transaction_hex>" },
  { "bitcoin": "<raw_transaction_hex>" }
]
```

### Output

Signed transactions are printed to stdout in the same JSON format:

```json
[
  { "bitcoin": "<signed_transaction_hex>" },
  { "bitcoin": "<signed_transaction_hex>" }
]
```

Progress information is printed to stderr.

## Example

```sh
# Using command line argument
sign-txs --bitcoind-container abc123 txs.json > signed.json

# Using environment variable
export BITCOIND_CONTAINER=abc123
sign-txs txs.json > signed.json
```

## License

MIT

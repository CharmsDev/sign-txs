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

## Setup

### Quick and Easy

Create a `.env` file in `containers/` (see `.env.example`). Run `docker compose up -d`.

### Detailed

The [`containers/`](containers/) directory provides Docker configurations for both the nginx proxy and bitcoind signer. Configure and run both services:

```sh
cd containers
cp .env.example .env
# Edit .env with your QuickNode endpoint URL and host
docker compose up -d
```

Then create a wallet:

```sh
docker exec -it bitcoind-signer bitcoin-cli createwallet "signing-wallet"
```

You can use the wallet as the signing wallet now.

### Connecting bitcoin-cli to a Remote Node

To use `bitcoin-cli` with a remote Bitcoin node (such as QuickNode), you can set up an nginx reverse proxy. This allows `bitcoin-cli` to connect to a local port that forwards requests to the remote node.

The [`containers/nginx/`](containers/nginx/) directory contains a Dockerfile and configuration template for the proxy. Configure it via environment variables:

```sh
docker build -t bitcoin-proxy containers/nginx/
docker run -d --name bitcoin-proxy -p 8332:8332 \
  -e PROXY_PASS_URL=https://xxx-xxxxx-xxxx.btc.quiknode.pro/ACCESS_TOKEN/ \
  -e PROXY_HOST=xxx-xxxxx-xxxx.btc.quiknode.pro \
  bitcoin-proxy
```

Now `bitcoin-cli` can connect to the remote node:

```sh
bitcoin-cli -rpcconnect=127.0.0.1 -rpcport=8332 getblockcount
```

### Running bitcoind in a Docker Container

For signing transactions, you need a bitcoind instance with a wallet. This can run in offline mode (no network connections) for security.

The [`containers/bitcoind/`](containers/bitcoind/) directory contains a ready-to-use Dockerfile and bitcoin.conf for an offline signing container.

Build and run the container:

```sh
# Build the image
docker build -t bitcoind-signer containers/bitcoind/

# Run the container
docker run -d --name bitcoind-signer bitcoind-signer

# Create or load a wallet
docker exec -it bitcoind-signer bitcoin-cli createwallet "signing-wallet"

# Import your private keys or descriptors
docker exec -it bitcoind-signer bitcoin-cli importdescriptors '[...]'
```

Use the container name with sign-txs:

```sh
sign-txs --bitcoind-container bitcoind-signer txs.json > signed.json
```

## License

MIT

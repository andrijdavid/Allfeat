#!/usr/bin/env bash
#
# Generate (rotate) fresh session keys on a RUNNING Allfeat node and return BOTH
# the public session keys and the ownership proof required by `session.setKeys`.
#
# Since pallet-session v46 (proof-of-possession), `set_keys` no longer accepts an
# empty/dummy proof (`0x00`): each session key must sign the *owner* account, i.e.
# the account that will submit `session.setKeys`. This prevents a front-runner
# from claiming ownership of someone else's keys (rogue-key attack). The node
# builds that proof for you through the `author_rotateKeysWithOwner` RPC, but it
# needs to know the owner account.
#
# Usage:
#   ./rotate_node_keys.sh <VALIDATOR_ACCOUNT> [RPC_URL]
#
#   VALIDATOR_ACCOUNT  Account that will sign `session.setKeys`, given as an SS58
#                      address (e.g. 5Fhc...) or a 0x-prefixed 32-byte public key.
#   RPC_URL            Node RPC endpoint (default: http://localhost:9944).
#
# Notes:
#   * `author_rotateKeysWithOwner` is an UNSAFE RPC. Call it on your own node over
#     localhost (allowed by default), or start the node with `--rpc-methods unsafe`.
#   * The proof is valid ONLY for this owner account: you must sign `setKeys` with
#     the exact same account you pass here.
#   * Pass ALLFEAT_BIN to point at the node binary; it is only needed to decode an
#     SS58 address (not required when you already pass a 0x public key).

set -euo pipefail

OWNER_INPUT="${1:-}"
RPC_URL="${2:-http://localhost:9944}"
BIN="${ALLFEAT_BIN:-allfeat}"

if [[ -z "$OWNER_INPUT" ]]; then
  echo "Usage: $0 <VALIDATOR_ACCOUNT (SS58 or 0x pubkey)> [RPC_URL]" >&2
  exit 1
fi

# --- Resolve the owner account to its raw 32-byte account id (0x + 64 hex) -----
if [[ "$OWNER_INPUT" =~ ^0x[0-9a-fA-F]{64}$ ]]; then
  OWNER_HEX="$OWNER_INPUT"
else
  if ! command -v "$BIN" >/dev/null 2>&1; then
    echo "Error: '$BIN' not found, cannot decode the SS58 address." >&2
    echo "       Pass the 0x public key directly, or set ALLFEAT_BIN to the node binary." >&2
    exit 1
  fi
  OWNER_HEX=$("$BIN" key inspect "$OWNER_INPUT" 2>/dev/null | grep "Account ID" | awk '{print $3}')
  if [[ ! "$OWNER_HEX" =~ ^0x[0-9a-fA-F]{64}$ ]]; then
    echo "Error: could not resolve '$OWNER_INPUT' to a 32-byte account id." >&2
    exit 1
  fi
fi

# --- Ask the node to generate fresh keys + ownership proof for this owner ------
RESPONSE=$(curl -s -H "Content-Type: application/json" \
  -d "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"author_rotateKeysWithOwner\",\"params\":[\"$OWNER_HEX\"]}" \
  "$RPC_URL")

KEYS=$(echo "$RESPONSE" | jq -r '.result.keys // empty')
PROOF=$(echo "$RESPONSE" | jq -r '.result.proof // empty')

if [[ -z "$KEYS" || -z "$PROOF" ]]; then
  echo "Error: unexpected RPC response (RPC unreachable, or unsafe methods disabled?):" >&2
  echo "$RESPONSE" >&2
  exit 1
fi

# SessionKeys = (grandpa: ed25519, aura: sr25519): 32 bytes (64 hex) each.
HEX=${KEYS:2}
if [[ ${#HEX} -ne 128 ]]; then
  echo "Warning: expected 2 session keys (128 hex chars), got ${#HEX}. Printing raw blob only." >&2
  echo "keys:  $KEYS"
  echo "proof: $PROOF"
  exit 0
fi

cat <<EOF
Owner account (must sign setKeys): $OWNER_HEX

Grandpa Public Key: 0x${HEX:0:64}
Aura Public Key:    0x${HEX:64:64}

==> Submit session.setKeys SIGNED BY THE OWNER ACCOUNT ABOVE, with:
  keys:  $KEYS
  proof: $PROOF
EOF

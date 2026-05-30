#!/usr/bin/env bash
#
# Generate a DETERMINISTIC set of Allfeat session keys from a single secret seed,
# insert them into the node keystore, and (optionally) emit the public keys plus
# the ownership proof needed by `session.setKeys`.
#
# This is the "deterministic / backup-able" alternative to rotate_node_keys.sh:
# you keep the seed, so the exact same keys can be re-derived later. The trade-off
# is that the ownership proof is produced OFFLINE from the seed here, because
# `author_rotateKeysWithOwner` would generate *new* random keys instead.
#
# Since pallet-session v46 (proof-of-possession), each session key must sign the
# statement `b"POP_" || owner`, where `owner` is the 32-byte account id that will
# submit `session.setKeys`. The proof is the SCALE tuple of those signatures, in
# the same order as the runtime `SessionKeys` struct: (grandpa, aura).
#
# Usage:
#   ./setup_validator_keys.sh [VALIDATOR_ACCOUNT]
#
#   VALIDATOR_ACCOUNT  (optional) Account that will sign `session.setKeys`, as an
#                      SS58 address or a 0x public key. When provided, the script
#                      also prints `keys` and `proof` ready for `session.setKeys`.
#
# Env:
#   NODE_PATH    Base path of the node whose keystore receives the keys. If unset,
#                key insertion is skipped (keys are only derived/printed).
#   ALLFEAT_BIN  Node binary (default: ./target/release/allfeat).

set -euo pipefail

BIN="${ALLFEAT_BIN:-./target/release/allfeat}"
OWNER_INPUT="${1:-}"

if [[ ! -x "$BIN" ]] && ! command -v "$BIN" >/dev/null 2>&1; then
  echo "Error: node binary '$BIN' not found." >&2
  echo "       Build it (cargo build --release) or set ALLFEAT_BIN." >&2
  exit 1
fi

# --- 1. Generate a single secret seed; ALL session keys derive from it ---------
RANDOM_SECRET=$("$BIN" key generate | grep "Secret phrase" | awk -F': ' '{print $2}' | sed 's/^ *//')

printf '=======================================================================================\n'
printf 'KEEP THIS SEED SAFE — IT GIVES FULL CONTROL OVER YOUR VALIDATOR SESSION KEYS\n'
printf '%s\n' "$RANDOM_SECRET"
printf '=======================================================================================\n\n'

# --- 2. Derive the public keys (order MUST match the runtime SessionKeys) -------
#        runtime SessionKeys = { grandpa: ed25519, aura: sr25519 }
GRANDPA_PUB=$("$BIN" key inspect --scheme ed25519 "$RANDOM_SECRET//grandpa" | grep "Account ID" | awk '{print $3}')
AURA_PUB=$("$BIN" key inspect --scheme sr25519 "$RANDOM_SECRET//aura" | grep "Account ID" | awk '{print $3}')

# --- 3. Insert the private keys into the node keystore --------------------------
if [[ -n "${NODE_PATH:-}" ]]; then
  "$BIN" key insert --base-path "$NODE_PATH" --scheme Ed25519 --suri "$RANDOM_SECRET//grandpa" --key-type gran
  "$BIN" key insert --base-path "$NODE_PATH" --scheme Sr25519 --suri "$RANDOM_SECRET//aura"    --key-type aura
  printf 'Inserted grandpa (gran) + aura keys into the keystore at %s\n\n' "$NODE_PATH"
else
  printf 'NODE_PATH not set — skipped keystore insertion (keys only derived).\n\n'
fi

printf 'Grandpa Public Key: %s\n' "$GRANDPA_PUB"
printf 'Aura Public Key:    %s\n\n' "$AURA_PUB"

# Concatenated public session keys = the `keys` argument of session.setKeys.
KEYS="0x${GRANDPA_PUB:2}${AURA_PUB:2}"

# --- 4. Optionally emit the ownership proof for the given owner account ---------
if [[ -z "$OWNER_INPUT" ]]; then
  printf 'No validator account passed: skipping ownership proof.\n'
  printf 'Concatenated session keys: %s\n' "$KEYS"
  printf 'Re-run as: %s <VALIDATOR_ACCOUNT>   to also get the setKeys proof.\n' "$0"
  exit 0
fi

# Resolve owner -> raw 32-byte account id (0x + 64 hex).
if [[ "$OWNER_INPUT" =~ ^0x[0-9a-fA-F]{64}$ ]]; then
  OWNER_HEX="$OWNER_INPUT"
else
  OWNER_HEX=$("$BIN" key inspect "$OWNER_INPUT" | grep "Account ID" | awk '{print $3}')
fi
if [[ ! "$OWNER_HEX" =~ ^0x[0-9a-fA-F]{64}$ ]]; then
  echo "Error: could not resolve owner '$OWNER_INPUT' to a 32-byte account id." >&2
  exit 1
fi

# Proof-of-possession statement = b"POP_" || owner   (0x504f505f == "POP_").
STATEMENT="0x504f505f${OWNER_HEX:2}"

# Each key signs the statement; proof = SCALE tuple (grandpa_sig, aura_sig),
# i.e. the two fixed 64-byte signatures concatenated, same order as SessionKeys.
GRANDPA_SIG=$("$BIN" key sign --hex --message "$STATEMENT" --scheme ed25519 --suri "$RANDOM_SECRET//grandpa")
AURA_SIG=$("$BIN" key sign --hex --message "$STATEMENT" --scheme sr25519 --suri "$RANDOM_SECRET//aura")
PROOF="0x${GRANDPA_SIG:2}${AURA_SIG:2}"

printf '\nOwner account (must sign setKeys): %s\n\n' "$OWNER_HEX"
printf '==> Submit session.setKeys SIGNED BY THE OWNER ACCOUNT ABOVE, with:\n'
printf '  keys:  %s\n' "$KEYS"
printf '  proof: %s\n' "$PROOF"

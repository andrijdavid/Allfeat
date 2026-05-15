#![cfg(test)]

//! Tests for the one-shot runtime migrations.
//!
//! These reproduce the production-like buggy state explicitly (rather than
//! relying on genesis, which is now correct) so they stay valid regardless of
//! future genesis changes, and exercise the actual `try-runtime`
//! `pre_upgrade`/`post_upgrade` hooks when built with `--features try-runtime`.

use crate::{tests::new_test_ext, *};
use frame_support::{assert_ok, traits::OnRuntimeUpgrade};
use pallet_token_allocation::{Allocations, EnvelopeId, Envelopes, NextAllocationId};
use shared_runtime::currency::AFT;
use sp_keyring::Sr25519Keyring;

/// Concrete stored types (the pallet does not derive `Debug`, so whole-struct
/// equality is asserted with `assert!(a == b)` rather than `assert_eq!`).
type Alloc = pallet_token_allocation::Allocation<AccountId, Balance, BlockNumber>;
type EnvCfg = pallet_token_allocation::EnvelopeConfig<Balance, BlockNumber, AccountId>;

const WRONG_CLIFF: BlockNumber = 18 * MONTHS;
const CORRECT_CLIFF: BlockNumber = 12 * MONTHS;

/// Recreate the exact on-chain state mainnet ended up in: a `Public2` envelope
/// with the erroneous 18-month cliff, plus allocations added via
/// `add_allocation(.., None)` so their `start` is baked to that wrong cliff.
fn make_buggy_public2(alice_amt: Balance, bob_amt: Balance) -> (u32, u32) {
    let cfg = Envelopes::<Runtime>::get(EnvelopeId::Public2).expect("Public2 envelope exists");
    assert_eq!(
        cfg.cliff, CORRECT_CLIFF,
        "fixture sanity: corrected genesis should give Public2 a 12mo cliff"
    );
    // Roll the envelope back to its historical erroneous state.
    Envelopes::<Runtime>::insert(
        EnvelopeId::Public2,
        pallet_token_allocation::EnvelopeConfig {
            cliff: WRONG_CLIFF,
            ..cfg
        },
    );

    let alice = Sr25519Keyring::Alice.to_account_id();
    let bob = Sr25519Keyring::Bob.to_account_id();

    let alice_id = NextAllocationId::<Runtime>::get();
    assert_ok!(pallet_token_allocation::Pallet::<Runtime>::add_allocation(
        RuntimeOrigin::root(),
        EnvelopeId::Public2,
        alice,
        alice_amt,
        None,
    ));
    let bob_id = NextAllocationId::<Runtime>::get();
    assert_ok!(pallet_token_allocation::Pallet::<Runtime>::add_allocation(
        RuntimeOrigin::root(),
        EnvelopeId::Public2,
        bob,
        bob_amt,
        None,
    ));

    // Confirm the bug is reproduced: `start` baked to the wrong 18mo cliff.
    for (id, amt) in [(alice_id, alice_amt), (bob_id, bob_amt)] {
        let a = Allocations::<Runtime>::get(id).expect("Public2 alloc exists");
        assert_eq!(a.envelope, EnvelopeId::Public2);
        assert_eq!(a.start, WRONG_CLIFF, "start must bake to the wrong cliff");
        assert_eq!(a.released, 0);
        assert_eq!(a.upfront, 0, "Public2 is 0% upfront");
        assert_eq!(a.total, amt);
        assert_eq!(a.vested_total, amt);
    }

    (alice_id, bob_id)
}

/// Assert the post-migration invariant: ONLY `Public2[*].start` (→ 0) and
/// `Envelopes[Public2].cliff` (→ 12mo) changed, plus the corrected vesting
/// behaviour.
fn assert_fixed(
    alice_id: u32,
    bob_id: u32,
    alice_amt: Balance,
    bob_amt: Balance,
    others_before: &[(u32, Alloc)],
    envs_before: &[(EnvelopeId, EnvCfg)],
) {
    // Envelope: cliff corrected, all other fields intact.
    let cfg = Envelopes::<Runtime>::get(EnvelopeId::Public2).unwrap();
    assert_eq!(cfg.cliff, CORRECT_CLIFF, "cliff must be lowered to 12mo");
    assert_eq!(cfg.total_cap, 75_000_000 * AFT);
    assert_eq!(cfg.upfront_rate, sp_runtime::Percent::from_percent(0));
    assert_eq!(cfg.vesting_duration, 12 * MONTHS);
    assert!(cfg.unique_beneficiary.is_none());

    // Public2 allocations: `start` zeroed, every other field unchanged.
    for (id, amt) in [(alice_id, alice_amt), (bob_id, bob_amt)] {
        let a = Allocations::<Runtime>::get(id).expect("Public2 alloc must still exist");
        assert_eq!(a.start, 0, "start must be reset to 0");
        assert_eq!(a.envelope, EnvelopeId::Public2);
        assert_eq!(a.total, amt);
        assert_eq!(a.vested_total, amt);
        assert_eq!(a.upfront, 0);
        assert_eq!(a.released, 0);
    }

    // Non-Public2 allocations: byte-for-byte unchanged, none added/removed.
    for (id, before) in others_before {
        let now = Allocations::<Runtime>::get(*id).expect("a control allocation vanished");
        assert!(now == *before, "a non-Public2 allocation was modified");
    }
    assert_eq!(
        Allocations::<Runtime>::iter().count(),
        others_before.len() + 2,
        "allocation count changed"
    );

    // Envelopes: only Public2.cliff changed; all others identical.
    for (eid, before) in envs_before {
        let now = Envelopes::<Runtime>::get(*eid).unwrap();
        if *eid == EnvelopeId::Public2 {
            assert!(
                now == EnvCfg {
                    cliff: CORRECT_CLIFF,
                    ..before.clone()
                },
                "a Public2 envelope field other than cliff changed"
            );
        } else {
            assert!(now == *before, "a non-Public2 envelope was modified");
        }
    }
    assert_eq!(Envelopes::<Runtime>::iter().count(), envs_before.len());

    // Behaviour: cliff now triggers at 12mo, full vest 12 months later.
    let a = Allocations::<Runtime>::get(alice_id).unwrap();
    assert_eq!(
        pallet_token_allocation::Pallet::<Runtime>::claimable_amount(&cfg, &a, CORRECT_CLIFF - 1),
        0,
        "nothing claimable strictly before the corrected 12mo cliff"
    );
    assert!(
        pallet_token_allocation::Pallet::<Runtime>::claimable_amount(&cfg, &a, 13 * MONTHS) > 0,
        "the fix unlocks vesting past the 12mo cliff (was 0 before the migration)"
    );
    assert_eq!(
        pallet_token_allocation::Pallet::<Runtime>::claimable_amount(
            &cfg,
            &a,
            CORRECT_CLIFF + 12 * MONTHS + 1
        ),
        a.vested_total,
        "fully vested 12 months after the corrected cliff"
    );
}

/// Storage + behavioural correctness of `on_runtime_upgrade`, plus the
/// idempotency guard. Runs under a plain `cargo test` (no feature needed).
#[test]
fn fix_public2_cliff_corrects_storage_and_is_idempotent() {
    new_test_ext().execute_with(|| {
        let alice_amt = 1_000_000 * AFT;
        let bob_amt = 2_000_000 * AFT;
        let (alice_id, bob_id) = make_buggy_public2(alice_amt, bob_amt);

        let others_before: Vec<(u32, Alloc)> = Allocations::<Runtime>::iter()
            .filter(|(_, a)| a.envelope != EnvelopeId::Public2)
            .collect();
        assert_eq!(
            others_before.len(),
            3,
            "expected the 3 Treasury auto-allocations as control"
        );
        let envs_before: Vec<(EnvelopeId, EnvCfg)> = Envelopes::<Runtime>::iter().collect();

        // Behavioural proof of the bug before fixing it.
        let cfg_buggy = Envelopes::<Runtime>::get(EnvelopeId::Public2).unwrap();
        let a_buggy = Allocations::<Runtime>::get(alice_id).unwrap();
        assert_eq!(
            pallet_token_allocation::Pallet::<Runtime>::claimable_amount(
                &cfg_buggy,
                &a_buggy,
                13 * MONTHS
            ),
            0,
            "bug: nothing claimable at 13mo, effective_start = max(18mo, 18mo)"
        );

        let noop = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

        let w = <crate::migrations::FixPublic2Cliff as OnRuntimeUpgrade>::on_runtime_upgrade();
        assert_ne!(
            w, noop,
            "migration must take the acting path, not the guarded no-op"
        );

        assert_fixed(
            alice_id,
            bob_id,
            alice_amt,
            bob_amt,
            &others_before,
            &envs_before,
        );

        // Idempotency: a second run is the guarded 1-read no-op.
        let w2 = <crate::migrations::FixPublic2Cliff as OnRuntimeUpgrade>::on_runtime_upgrade();
        assert_eq!(w2, noop, "second run must be the guarded no-op");
        assert_fixed(
            alice_id,
            bob_id,
            alice_amt,
            bob_amt,
            &others_before,
            &envs_before,
        );
    });
}

/// Full `try-runtime` path: `pre_upgrade` → `on_runtime_upgrade` →
/// `post_upgrade`, exercising the on-chain invariant checks the team will run
/// against forked mainnet state. Requires `--features try-runtime`.
#[cfg(feature = "try-runtime")]
#[test]
fn fix_public2_cliff_try_runtime_hooks_pass() {
    new_test_ext().execute_with(|| {
        let alice_amt = 1_000_000 * AFT;
        let bob_amt = 2_000_000 * AFT;
        let (alice_id, bob_id) = make_buggy_public2(alice_amt, bob_amt);

        let others_before: Vec<(u32, Alloc)> = Allocations::<Runtime>::iter()
            .filter(|(_, a)| a.envelope != EnvelopeId::Public2)
            .collect();
        let envs_before: Vec<(EnvelopeId, EnvCfg)> = Envelopes::<Runtime>::iter().collect();

        type M = crate::migrations::FixPublic2Cliff;
        let state = <M as OnRuntimeUpgrade>::pre_upgrade().expect("pre_upgrade must succeed");
        let _ = <M as OnRuntimeUpgrade>::on_runtime_upgrade();
        <M as OnRuntimeUpgrade>::post_upgrade(state).expect("post_upgrade invariants must hold");

        assert_fixed(
            alice_id,
            bob_id,
            alice_amt,
            bob_amt,
            &others_before,
            &envs_before,
        );
    });
}

/// Regression test for the failure the real `try-runtime-cli` surfaced: in its
/// idempotency and MBM ("execute the block one more time") passes, try-runtime
/// runs the migration and then re-invokes the WHOLE upgrade — including
/// `pre_upgrade` — on the already-migrated state. `pre_upgrade` must therefore
/// not hard-fail when the cliff is already corrected; the end-state invariant
/// in `post_upgrade` must still hold on the no-op pass. Requires
/// `--features try-runtime`.
#[cfg(feature = "try-runtime")]
#[test]
fn fix_public2_cliff_try_runtime_hooks_are_repeat_safe() {
    new_test_ext().execute_with(|| {
        let alice_amt = 1_000_000 * AFT;
        let bob_amt = 2_000_000 * AFT;
        let (alice_id, bob_id) = make_buggy_public2(alice_amt, bob_amt);

        let others_before: Vec<(u32, Alloc)> = Allocations::<Runtime>::iter()
            .filter(|(_, a)| a.envelope != EnvelopeId::Public2)
            .collect();
        let envs_before: Vec<(EnvelopeId, EnvCfg)> = Envelopes::<Runtime>::iter().collect();

        type M = crate::migrations::FixPublic2Cliff;

        // Pass 1: the genuine migration (state is in the erroneous shape).
        let s1 = <M as OnRuntimeUpgrade>::pre_upgrade().expect("pre_upgrade pass 1");
        let _ = <M as OnRuntimeUpgrade>::on_runtime_upgrade();
        <M as OnRuntimeUpgrade>::post_upgrade(s1).expect("post_upgrade pass 1");
        assert_fixed(
            alice_id,
            bob_id,
            alice_amt,
            bob_amt,
            &others_before,
            &envs_before,
        );

        // Pass 2: re-run the full hook cycle on the ALREADY-migrated state,
        // exactly as try-runtime's MBM "execute the block one more time" does.
        // This is the scenario that panicked on real mainnet state.
        let s2 = <M as OnRuntimeUpgrade>::pre_upgrade()
            .expect("pre_upgrade must tolerate the already-corrected cliff");
        let _ = <M as OnRuntimeUpgrade>::on_runtime_upgrade();
        <M as OnRuntimeUpgrade>::post_upgrade(s2)
            .expect("post_upgrade end-state invariant must still hold on the no-op pass");
        assert_fixed(
            alice_id,
            bob_id,
            alice_amt,
            bob_amt,
            &others_before,
            &envs_before,
        );
    });
}

// This file is part of Allfeat.

// Copyright (C) 2022-2025 Allfeat.
// SPDX-License-Identifier: GPL-3.0-or-later

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! One-shot runtime migrations for the mainnet runtime.

use crate::{MONTHS, Runtime};
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;
use pallet_token_allocation::{Allocations, EnvelopeId, Envelopes};

#[cfg(feature = "try-runtime")]
use alloc::vec::Vec;

/// The set of migrations applied on the next runtime upgrade, in order.
pub type Migrations = (FixPublic2Cliff,);

/// Corrective migration for the `Public2` envelope.
///
/// At genesis the `Public2` envelope was configured with an **18-month** cliff,
/// whereas the contributor contracts stipulate a **12-month** cliff. Allocations
/// were added post-launch via `add_allocation` with `start = None`, so each
/// allocation's `start` field was baked to the (wrong) envelope cliff value
/// `18 * MONTHS = 7_776_000`.
///
/// Because `claimable_amount` computes `effective_start = max(alloc.start,
/// cfg.cliff)`, fixing this requires BOTH:
///   1. lowering `Envelopes[Public2].cliff` to `12 * MONTHS`, and
///   2. resetting every `Public2` allocation's `start` to `0`
///
/// so that `max(0, 12 * MONTHS) = 12 * MONTHS`, i.e. the cliff is anchored on
/// chain genesis (consistent with every other envelope). `upfront_rate` is 0%
/// and nothing has been released yet (run while still inside the cliff), so no
/// funds are affected — this is a purely forward-looking correction.
///
/// The migration is guarded: it only acts while the on-chain cliff is still the
/// erroneous `18 * MONTHS`, making it idempotent and safe to leave in the tuple.
pub struct FixPublic2Cliff;

/// Erroneous cliff currently stored on-chain for `Public2` (18 months).
const WRONG_CLIFF: crate::BlockNumber = 18 * MONTHS;
/// Corrected cliff per the signed contributor contracts (12 months).
const CORRECT_CLIFF: crate::BlockNumber = 12 * MONTHS;

/// Concrete `Allocation` type as stored by the runtime, used to snapshot and
/// byte-for-byte compare state across the upgrade under `try-runtime`.
#[cfg(feature = "try-runtime")]
type SnapAlloc =
    pallet_token_allocation::Allocation<crate::AccountId, crate::Balance, crate::BlockNumber>;
/// Concrete `EnvelopeConfig` type as stored by the runtime (see [`SnapAlloc`]).
#[cfg(feature = "try-runtime")]
type SnapEnv =
    pallet_token_allocation::EnvelopeConfig<crate::Balance, crate::BlockNumber, crate::AccountId>;

impl OnRuntimeUpgrade for FixPublic2Cliff {
    fn on_runtime_upgrade() -> Weight {
        let db = <Runtime as frame_system::Config>::DbWeight::get();

        // Guard: read the current Public2 envelope. Bail out (no-op) unless the
        // erroneous 18-month cliff is still in place. Makes the migration
        // idempotent if it is run more than once or left in the tuple.
        let cfg = match Envelopes::<Runtime>::get(EnvelopeId::Public2) {
            Some(cfg) if cfg.cliff == WRONG_CLIFF => cfg,
            _ => return db.reads(1),
        };

        // 1. Collect the ids of every Public2 allocation.
        //
        // We collect first to avoid mutating the map while a lazy iterator over
        // it is live. `iter()` is a full scan, so we also record exactly how
        // many entries were read for accurate weight accounting. Allocation
        // count is small (one entry per contributor), so an in-memory Vec is
        // fine for a one-shot upgrade.
        let mut total_iterated: u64 = 0;
        let mut ids: alloc::vec::Vec<u32> = alloc::vec::Vec::new();
        for (id, alloc) in Allocations::<Runtime>::iter() {
            total_iterated = total_iterated.saturating_add(1);
            if alloc.envelope == EnvelopeId::Public2 {
                ids.push(id);
            }
        }

        let touched = ids.len() as u64;

        // 2. Reset `start` to 0 for every Public2 allocation.
        for id in ids {
            Allocations::<Runtime>::mutate(id, |maybe_alloc| {
                if let Some(alloc) = maybe_alloc {
                    alloc.start = 0;
                }
            });
        }

        // 3. Lower the envelope cliff to the contractually-correct 12 months.
        Envelopes::<Runtime>::insert(
            EnvelopeId::Public2,
            pallet_token_allocation::EnvelopeConfig {
                cliff: CORRECT_CLIFF,
                ..cfg
            },
        );

        log::info!(
            target: "runtime::migration",
            "FixPublic2Cliff: cliff {WRONG_CLIFF} -> {CORRECT_CLIFF}, reset start on {touched} allocation(s)",
        );

        // Reads: 1 (envelope guard) + `total_iterated` (the full Allocations
        // scan) + `touched` (each `mutate` reads before writing).
        // Writes: `touched` (each `mutate`) + 1 (the envelope insert).
        db.reads_writes(
            total_iterated.saturating_add(touched).saturating_add(1),
            touched.saturating_add(1),
        )
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
        use parity_scale_codec::Encode;

        let cfg = Envelopes::<Runtime>::get(EnvelopeId::Public2)
            .ok_or("pre_upgrade: Public2 envelope missing")?;
        // `pre_upgrade` must be repeat-safe: try-runtime runs the migration via
        // block production and then re-invokes the whole upgrade with
        // pre/post checks on the *already-migrated* state (the idempotency
        // and MBM "execute the block one more time" passes). So accept BOTH
        // the erroneous 18-month cliff (genuine first run) and the
        // already-corrected 12-month cliff (no-op run); only a third value is
        // unexpected. The real guarantee is the end-state invariant asserted
        // unconditionally in `post_upgrade`.
        frame_support::ensure!(
            cfg.cliff == WRONG_CLIFF || cfg.cliff == CORRECT_CLIFF,
            "pre_upgrade: Public2 cliff is neither the erroneous 18*MONTHS nor the corrected 12*MONTHS value"
        );

        // Snapshot the *entire* allocation set (split by envelope) and every
        // envelope config, so `post_upgrade` can prove the migration touched
        // ONLY `Public2[*].start` and `Envelopes[Public2].cliff`, and nothing
        // else — important for a migration that moves money's unlock schedule.
        let mut public2: alloc::vec::Vec<(u32, SnapAlloc)> = alloc::vec::Vec::new();
        let mut others: alloc::vec::Vec<(u32, SnapAlloc)> = alloc::vec::Vec::new();
        for (id, a) in Allocations::<Runtime>::iter() {
            if a.envelope == EnvelopeId::Public2 {
                public2.push((id, a));
            } else {
                others.push((id, a));
            }
        }
        let envelopes: alloc::vec::Vec<(EnvelopeId, SnapEnv)> =
            Envelopes::<Runtime>::iter().collect();

        Ok((public2, others, envelopes).encode())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
        use parity_scale_codec::Decode;

        type Snapshot = (
            alloc::vec::Vec<(u32, SnapAlloc)>,
            alloc::vec::Vec<(u32, SnapAlloc)>,
            alloc::vec::Vec<(EnvelopeId, SnapEnv)>,
        );
        let (pre_public2, pre_others, pre_envelopes): Snapshot = Decode::decode(&mut &state[..])
            .map_err(|_| "post_upgrade: failed to decode pre-state")?;

        // 1. Public2 envelope: cliff lowered to 12*MONTHS, nothing else changed.
        let new_cfg = Envelopes::<Runtime>::get(EnvelopeId::Public2)
            .ok_or("post_upgrade: Public2 envelope missing")?;
        frame_support::ensure!(
            new_cfg.cliff == CORRECT_CLIFF,
            "post_upgrade: Public2 cliff was not lowered to 12*MONTHS"
        );
        let pre_public2_env = pre_envelopes
            .iter()
            .find_map(|(eid, c)| (*eid == EnvelopeId::Public2).then(|| c.clone()))
            .ok_or("post_upgrade: Public2 missing from pre-state envelopes")?;
        frame_support::ensure!(
            new_cfg
                == SnapEnv {
                    cliff: CORRECT_CLIFF,
                    ..pre_public2_env
                },
            "post_upgrade: a Public2 envelope field other than `cliff` changed"
        );

        // 2. Every other envelope is byte-for-byte unchanged.
        for (eid, pre) in pre_envelopes
            .iter()
            .filter(|(e, _)| *e != EnvelopeId::Public2)
        {
            let now = Envelopes::<Runtime>::get(*eid)
                .ok_or("post_upgrade: a non-Public2 envelope disappeared")?;
            frame_support::ensure!(
                now == *pre,
                "post_upgrade: a non-Public2 envelope was modified"
            );
        }
        frame_support::ensure!(
            Envelopes::<Runtime>::iter().count() == pre_envelopes.len(),
            "post_upgrade: the number of envelopes changed"
        );

        // 3. Every Public2 allocation: only `start` changed, and it is now 0.
        for (id, pre) in &pre_public2 {
            let now = Allocations::<Runtime>::get(*id)
                .ok_or("post_upgrade: a Public2 allocation disappeared")?;
            frame_support::ensure!(
                now.start == 0,
                "post_upgrade: a Public2 allocation still has a non-zero start"
            );
            frame_support::ensure!(
                now == SnapAlloc {
                    start: 0,
                    ..pre.clone()
                },
                "post_upgrade: a Public2 allocation field other than `start` changed"
            );
        }

        // 4. Every non-Public2 allocation is byte-for-byte unchanged.
        for (id, pre) in &pre_others {
            let now = Allocations::<Runtime>::get(*id)
                .ok_or("post_upgrade: a non-Public2 allocation disappeared")?;
            frame_support::ensure!(
                now == *pre,
                "post_upgrade: a non-Public2 allocation was modified"
            );
        }

        // 5. No allocation was added or removed.
        frame_support::ensure!(
            Allocations::<Runtime>::iter().count() == pre_public2.len() + pre_others.len(),
            "post_upgrade: the total allocation count changed"
        );

        Ok(())
    }
}

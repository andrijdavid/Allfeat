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

//! Layer 4 of `../midds-sdk/docs/testing.md` — runtime integration tests.
//!
//! Exercises `pallet_midds<Instance1>` (the `MusicalWorks` instance) on the
//! real `melodie-runtime`, including the full economic model layered in by
//! `../midds-sdk/docs/economics.md`: multipliers, refundable window,
//! finalize-to-Treasury, and the sudo split.
//!
//! The fees-report aggregate lives in `target/test-reports/fees_report.md` —
//! commit-able artefact that anchors decisions on tuning `DepositBase` /
//! `DepositPerByte` (cf. `docs/testing.md` §7.4). Numbers are computed at
//! the *minimum* multiplier (no congestion) — multiply the fee columns by
//! the runtime's live `M_fast × M_slow` for actual conditions.
//!
//! Plumbed against `melodie-runtime` here (not `midds-sdk`) to preserve the
//! SDK / runtime decoupling: `midds-sdk` never depends on `melodie-runtime`.

use crate::{
    AccountId, Balance, Balances, MiddsCommitmentWindow, MiddsTreasuryAccount, Runtime,
    RuntimeCall, RuntimeOrigin, TransactionByteFee, WeightToFee,
};
use frame_support::{
    traits::{
        Hooks,
        fungible::{Inspect, InspectHold},
    },
    weights::WeightToFee as WeightToFeeTrait,
};
use midds_fixtures::{gen_n, pathological};
use midds_types::MusicalWork;
use pallet_midds::weights::WeightInfo;
use parity_scale_codec::Encode;
use shared_runtime::currency::AFT;
use sp_runtime::BuildStorage;
use std::{fs, path::PathBuf};

// -----------------------------------------------------------------------------
// Externalities builder
// -----------------------------------------------------------------------------

const FUND_PER_ACCOUNT: Balance = 1_000_000 * AFT;

fn account(nonce: u32) -> AccountId {
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(&nonce.to_be_bytes());
    AccountId::new(bytes)
}

fn build_ext(accounts: &[AccountId]) -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .expect("frame_system genesis");
    pallet_balances::GenesisConfig::<Runtime> {
        balances: accounts
            .iter()
            .cloned()
            .map(|a| (a, FUND_PER_ACCOUNT))
            .collect(),
        ..Default::default()
    }
    .assimilate_storage(&mut t)
    .expect("balances genesis");
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| frame_system::Pallet::<Runtime>::set_block_number(1));
    ext
}

/// Fast-forward block production with the pallet's `on_initialize` firing —
/// required to land finalizations and roll the multiplier windows.
fn advance_to(target: crate::BlockNumber) {
    let mut now = frame_system::Pallet::<Runtime>::block_number();
    while now < target {
        now += 1;
        frame_system::Pallet::<Runtime>::set_block_number(now);
        <pallet_midds::Pallet<Runtime, pallet_midds::Instance1> as Hooks<crate::BlockNumber>>::on_initialize(now);
    }
}

// -----------------------------------------------------------------------------
// Fee math (analytical, mirroring pallet_transaction_payment::compute_fee)
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct FeeBreakdown {
    encoded_size: u32,
    encoded_call_len: u32,
    /// Unmultiplied bond — `DepositBase + DepositPerByte * encoded_size`.
    /// What `remove_own` would refund. The total held bond at deposit time
    /// is `bond * M_fast * M_slow`; reports stay at the *neutral*
    /// multiplier (1.0×) since multipliers depend on chain state.
    bond: Balance,
    weight_fee: Balance,
    length_fee: Balance,
}

impl FeeBreakdown {
    fn total_min(&self) -> Balance {
        self.bond
            .saturating_add(self.weight_fee)
            .saturating_add(self.length_fee)
    }
}

fn measure(item: &MusicalWork) -> FeeBreakdown {
    let encoded_size = item.encoded_size() as u32;
    let call = RuntimeCall::MusicalWorks(pallet_midds::Call::deposit { item: item.clone() });
    let encoded_call_len = call.encode().len() as u32;
    // Runtime wires `WeightInfo = ()` for the MIDDS pallet (no benchmarked
    // weights yet). The zero-weight impl returns `Weight::zero()`, which —
    // multiplied by `WeightToFee` — produces a 0 AFT weight fee. The
    // analytical column accordingly reads "-" until benchmarks land and
    // the runtime swaps in real numbers; the report still prints the bond
    // and length-fee columns truthfully, which is what tuning depends on.
    let weight_fee = <WeightToFee as WeightToFeeTrait>::weight_to_fee(
        &<() as WeightInfo>::deposit(encoded_size),
    );
    let length_fee = TransactionByteFee::get().saturating_mul(encoded_call_len as Balance);
    FeeBreakdown {
        encoded_size,
        encoded_call_len,
        bond: bond_for_size(encoded_size),
        weight_fee,
        length_fee,
    }
}

fn bond_for_size(size: u32) -> Balance {
    let base = <Runtime as pallet_midds::Config<pallet_midds::Instance1>>::DepositBase::get();
    let per_byte =
        <Runtime as pallet_midds::Config<pallet_midds::Instance1>>::DepositPerByte::get();
    base.saturating_add(per_byte.saturating_mul(size as Balance))
}

// -----------------------------------------------------------------------------
// "Avg" payload — kept here rather than in `midds-fixtures` because tuning
// is melodie-specific. Targets ~200 bytes encoded (representative of a track
// with a handful of creators), matching `docs/testing.md` §7.3.
// -----------------------------------------------------------------------------

fn avg_size_musical_work() -> MusicalWork {
    use frame_support::BoundedVec;
    use midds_fixtures::identifiers::{ipi_from_stem, iswc_for_index};
    use midds_types::{Creator, CreatorId, CreatorRole, Language, MusicalWorkV1, WorkType};

    let title = BoundedVec::try_from(vec![b'a'; 64]).expect("title fits");
    let creators = (0..4u32)
        .map(|i| Creator {
            role: match i % 4 {
                0 => CreatorRole::Composer,
                1 => CreatorRole::Author,
                2 => CreatorRole::Arranger,
                _ => CreatorRole::Publisher,
            },
            id: CreatorId::Ipi(ipi_from_stem(u64::from(123_456_789 + i), 11)),
        })
        .collect::<Vec<_>>();
    let creators = BoundedVec::try_from(creators).expect("4 creators fit");

    MusicalWork::V1(MusicalWorkV1 {
        iswc: iswc_for_index(424_242),
        title,
        creation_year: 2024,
        instrumental: false,
        language: Some(Language::En),
        bpm: Some(120),
        key: None,
        work_type: WorkType::Original,
        creators,
        classical_info: None,
        offchain_extension: None,
    })
}

// -----------------------------------------------------------------------------
// Real on-chain execution — proves the analytical numbers against actual
// `MutateHold` accounting on the runtime.
// -----------------------------------------------------------------------------

/// Execute `deposit(item)` and assert the held bond matches the analytical
/// formula bit-for-bit. At fresh chain state the multipliers are 1.0×, so
/// the held amount equals the unmultiplied base.
fn assert_real_bond_matches(item: &MusicalWork) {
    let depositor = account(1);
    let breakdown = measure(item);

    let mut ext = build_ext(&[depositor.clone()]);
    ext.execute_with(|| {
        pallet_midds::Pallet::<Runtime, pallet_midds::Instance1>::deposit(
            RuntimeOrigin::signed(depositor.clone()),
            item.clone(),
        )
        .expect("deposit on melodie-runtime");

        let held = <Balances as InspectHold<AccountId>>::balance_on_hold(
            &pallet_midds::HoldReason::<pallet_midds::Instance1>::Deposit.into(),
            &depositor,
        );
        assert_eq!(
            held, breakdown.bond,
            "held bond drift for {} bytes payload",
            breakdown.encoded_size
        );
    });
}

// -----------------------------------------------------------------------------
// Reports
// -----------------------------------------------------------------------------

fn report_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("test-reports")
}

fn format_aft(balance: Balance) -> String {
    if balance == 0 {
        return "-".into();
    }
    let aft = balance / AFT;
    let micro = (balance % AFT) / 1_000_000;
    format!("{aft}.{micro:06} AFT")
}

struct ReportRow {
    label: &'static str,
    breakdown: FeeBreakdown,
}

fn write_markdown_report(rows: &[ReportRow], distribution: &DistributionSummary) {
    let dir = report_dir();
    fs::create_dir_all(&dir).expect("create target/test-reports");
    let path = dir.join("fees_report.md");

    let mut s = String::new();
    s.push_str("# MIDDS — Fees Report (melodie-runtime)\n\n");
    s.push_str(
        "Generated by `cargo test -p melodie-runtime tests::midds_integration::fees_report`. \
         Numbers are at the *minimum* fee multiplier (no congestion) **and** the neutral 1.0× \
         deposit multipliers — multiply the fee + bond columns by the runtime's live \
         `M_fast × M_slow` (queryable via `midds_currentMultipliers`) for actual conditions.\n\n",
    );
    s.push_str(
        "| Scenario | Size (bytes) | Encoded call (bytes) | Bond | Weight fee | Length fee | Total user cost |\n",
    );
    s.push_str(
        "|----------|-------------:|---------------------:|-----:|-----------:|-----------:|----------------:|\n",
    );
    for ReportRow { label, breakdown } in rows {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            label,
            breakdown.encoded_size,
            breakdown.encoded_call_len,
            format_aft(breakdown.bond),
            format_aft(breakdown.weight_fee),
            format_aft(breakdown.length_fee),
            format_aft(breakdown.total_min()),
        ));
    }

    s.push_str("\n## Distribution over 1 000 generated payloads\n\n");
    s.push_str(
        "Total user cost (bond + weight fee + length fee at min multiplier), sampled from \
         `midds_fixtures::gen_n` (deterministic seed). Useful as a baseline for tuning \
         `DepositBase` / `DepositPerByte`.\n\n",
    );
    s.push_str("| Percentile | Total user cost |\n");
    s.push_str("|-----------:|----------------:|\n");
    s.push_str(&format!("| min | {} |\n", format_aft(distribution.min)));
    s.push_str(&format!("| p50 | {} |\n", format_aft(distribution.p50)));
    s.push_str(&format!("| p95 | {} |\n", format_aft(distribution.p95)));
    s.push_str(&format!("| p99 | {} |\n", format_aft(distribution.p99)));
    s.push_str(&format!("| max | {} |\n", format_aft(distribution.max)));

    fs::write(&path, s).expect("write fees_report.md");
    eprintln!("wrote {}", path.display());
}

struct DistributionSummary {
    min: Balance,
    p50: Balance,
    p95: Balance,
    p99: Balance,
    max: Balance,
}

fn distribution_summary(mut totals: Vec<Balance>) -> DistributionSummary {
    assert!(!totals.is_empty(), "non-empty distribution");
    totals.sort_unstable();
    let n = totals.len();
    let pick = |q: f64| -> Balance {
        let idx = ((q * n as f64).ceil() as usize)
            .saturating_sub(1)
            .min(n - 1);
        totals[idx]
    };
    DistributionSummary {
        min: totals[0],
        p50: pick(0.50),
        p95: pick(0.95),
        p99: pick(0.99),
        max: totals[n - 1],
    }
}

// -----------------------------------------------------------------------------
// Tests — bond shape (regression on the analytical formula).
// -----------------------------------------------------------------------------

const DISTRIBUTION_SEED: u64 = 0xCAFE_BABE_F005_BA11;

#[test]
fn fees_small_musical_work_holds_correct_bond() {
    assert_real_bond_matches(&pathological::min_size_musical_work());
}

#[test]
fn fees_avg_musical_work_holds_correct_bond() {
    assert_real_bond_matches(&avg_size_musical_work());
}

#[test]
fn fees_max_musical_work_holds_correct_bond() {
    assert_real_bond_matches(&pathological::max_size_musical_work());
}

#[test]
fn fees_distribution_1000_executes() {
    let signers: Vec<AccountId> = (0..16u32).map(account).collect();
    let mut ext = build_ext(&signers);
    let items = gen_n(DISTRIBUTION_SEED, 1_000);
    ext.execute_with(|| {
        for (i, item) in items.iter().enumerate() {
            let signer = signers[i % signers.len()].clone();
            pallet_midds::Pallet::<Runtime, pallet_midds::Instance1>::deposit(
                RuntimeOrigin::signed(signer),
                item.clone(),
            )
            .unwrap_or_else(|e| panic!("deposit #{i} failed: {e:?}"));
        }
    });
}

#[test]
fn fees_report() {
    let rows = vec![
        ReportRow {
            label: "small (min payload)",
            breakdown: measure(&pathological::min_size_musical_work()),
        },
        ReportRow {
            label: "avg (~200 B target)",
            breakdown: measure(&avg_size_musical_work()),
        },
        ReportRow {
            label: "max (MaxEncodedLen)",
            breakdown: measure(&pathological::max_size_musical_work()),
        },
    ];

    let totals: Vec<Balance> = gen_n(DISTRIBUTION_SEED, 1_000)
        .iter()
        .map(|w| measure(w).total_min())
        .collect();
    let distribution = distribution_summary(totals);

    write_markdown_report(&rows, &distribution);

    assert!(
        rows[0].breakdown.bond < rows[1].breakdown.bond,
        "small bond ≥ avg bond: {} vs {}",
        rows[0].breakdown.bond,
        rows[1].breakdown.bond
    );
    assert!(
        rows[1].breakdown.bond < rows[2].breakdown.bond,
        "avg bond ≥ max bond: {} vs {}",
        rows[1].breakdown.bond,
        rows[2].breakdown.bond
    );
}

// -----------------------------------------------------------------------------
// Economics integration — exercises the new lifecycle on the real runtime.
// -----------------------------------------------------------------------------

/// `remove_own` within the window: depositor recovers the unmultiplied base
/// bond, the multiplier premium goes to the Treasury. We pin a non-trivial
/// premium by writing `M_fast = 2.0×` directly so the assertion exercises
/// the premium path even when the chain is otherwise idle.
#[test]
fn remove_own_refunds_base_premium_goes_to_treasury() {
    use sp_runtime::FixedU128;

    let depositor = account(1);
    let mut ext = build_ext(&[depositor.clone()]);
    ext.execute_with(|| {
        pallet_midds::FastMultiplier::<Runtime, pallet_midds::Instance1>::put(FixedU128::from_u32(
            2,
        ));

        let item = pathological::min_size_musical_work();
        let base = bond_for_size(item.encoded_size() as u32);
        let total = base.saturating_mul(2);
        let free_before = <Balances as Inspect<AccountId>>::balance(&depositor);
        let treasury_before =
            <Balances as Inspect<AccountId>>::balance(&MiddsTreasuryAccount::get());

        pallet_midds::Pallet::<Runtime, pallet_midds::Instance1>::deposit(
            RuntimeOrigin::signed(depositor.clone()),
            item,
        )
        .expect("deposit");
        assert_eq!(
            <Balances as InspectHold<AccountId>>::balance_on_hold(
                &pallet_midds::HoldReason::<pallet_midds::Instance1>::Deposit.into(),
                &depositor,
            ),
            total,
        );

        pallet_midds::Pallet::<Runtime, pallet_midds::Instance1>::remove_own(
            RuntimeOrigin::signed(depositor.clone()),
            0,
        )
        .expect("remove_own");

        let free_after = <Balances as Inspect<AccountId>>::balance(&depositor);
        let treasury_after =
            <Balances as Inspect<AccountId>>::balance(&MiddsTreasuryAccount::get());

        let premium = total - base;
        assert_eq!(
            free_after,
            free_before - premium,
            "depositor must keep `base` and lose only the premium",
        );
        assert_eq!(
            treasury_after,
            treasury_before + premium,
            "Treasury must receive the multiplier premium",
        );
    });
}

/// `finalize` after the commitment window moves the *full* held bond to the
/// Treasury and flips `finalized` on the deposit info. Eager via the
/// `on_initialize` hook — the production path.
#[test]
fn finalize_via_hook_moves_bond_to_treasury() {
    let depositor = account(1);
    let mut ext = build_ext(&[depositor.clone()]);
    ext.execute_with(|| {
        let item = pathological::min_size_musical_work();
        let bond = bond_for_size(item.encoded_size() as u32);
        let treasury_before =
            <Balances as Inspect<AccountId>>::balance(&MiddsTreasuryAccount::get());

        pallet_midds::Pallet::<Runtime, pallet_midds::Instance1>::deposit(
            RuntimeOrigin::signed(depositor.clone()),
            item,
        )
        .expect("deposit");

        // CommitmentWindow is 7 days = 7 × DAYS blocks. Finalization fires
        // when `block > deposited_at + window`.
        let now = frame_system::Pallet::<Runtime>::block_number();
        advance_to(now + MiddsCommitmentWindow::get() + 1);

        assert_eq!(
            <Balances as InspectHold<AccountId>>::balance_on_hold(
                &pallet_midds::HoldReason::<pallet_midds::Instance1>::Deposit.into(),
                &depositor,
            ),
            0,
            "hold released after finalize",
        );
        assert_eq!(
            <Balances as Inspect<AccountId>>::balance(&MiddsTreasuryAccount::get()),
            treasury_before + bond,
            "Treasury credited with the full bond",
        );

        let info = pallet_midds::DepositInfo::<Runtime, pallet_midds::Instance1>::get(0)
            .expect("deposit info kept after finalize");
        assert!(info.finalized);
    });
}

/// Multi-claim: two distinct payloads sharing an ISWC are both indexed under
/// the same `IdentifierClaims` key, and the runtime API surface returns both
/// on a single lookup.
#[test]
fn multi_claim_lookup_returns_all_ids() {
    use frame_support::BoundedVec;
    use midds_fixtures::identifiers::iswc_for_index;
    use midds_types::{Creator, CreatorId, CreatorRole, MusicalWorkV1, WorkType};

    let alice = account(1);
    let bob = account(2);
    let mut ext = build_ext(&[alice.clone(), bob.clone()]);
    ext.execute_with(|| {
        // Two payloads with the *same* ISWC but a different title — the
        // payload-hash guard must reject only byte-identical re-deposits.
        let iswc = iswc_for_index(0);
        let make_work = |title: &[u8]| {
            MusicalWork::V1(MusicalWorkV1 {
                iswc: iswc.clone(),
                title: BoundedVec::try_from(title.to_vec()).expect("title fits"),
                creation_year: 2024,
                instrumental: false,
                language: None,
                bpm: None,
                key: None,
                work_type: WorkType::Original,
                creators: BoundedVec::try_from(vec![Creator {
                    role: CreatorRole::Composer,
                    id: CreatorId::Ipi(
                        BoundedVec::try_from(b"123456789".to_vec()).expect("9-byte"),
                    ),
                }])
                .expect("single creator"),
                classical_info: None,
                offchain_extension: None,
            })
        };

        pallet_midds::Pallet::<Runtime, pallet_midds::Instance1>::deposit(
            RuntimeOrigin::signed(alice),
            make_work(b"version A"),
        )
        .expect("first claim");
        pallet_midds::Pallet::<Runtime, pallet_midds::Instance1>::deposit(
            RuntimeOrigin::signed(bob),
            make_work(b"version B"),
        )
        .expect("second claim");

        let mut ids =
            pallet_midds::Pallet::<Runtime, pallet_midds::Instance1>::lookup_by_identifier(iswc);
        ids.sort();
        assert_eq!(ids, vec![0, 1]);
    });
}

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

use crate::*;
use frame_support::{PalletId, parameter_types};
use frame_system::{EnsureRoot, EnsureSigned};
use shared_runtime::currency::{MICROAFT, MILLIAFT};
use sp_runtime::{FixedU128, MultiSigner, traits::AccountIdConversion};

// MIDDS economic model — see `../midds-sdk/docs/economics.md`. Numbers below
// implement section 6 of that doc verbatim; all the rationale lives there.
parameter_types! {
    // Bond formula (unmultiplied) — calibrated to ~$0.01 in regime nominal at
    // ~$0.02/AFT. The dynamic multipliers stack on top via `M_fast × M_slow`.
    pub const MiddsDepositBase: Balance = 500 * MILLIAFT;
    pub const MiddsDepositPerByte: Balance = 10 * MICROAFT;

    // Refundable commitment window aligned with the IFPI Friday Global
    // Release Day. Within this window the depositor can `remove_own` (base
    // refunded, premium → Treasury) or `update`. After: bond → Treasury via
    // `finalize` (eager hook + permissionless catch-up).
    pub const MiddsCommitmentWindow: BlockNumber = 7 * DAYS;
    pub const MiddsMaxFinalizationsPerBlock: u32 = 100;
    // Sudo-only cleanup cap — bounds `force_remove_many` weight. Set ≥ the
    // benchmark sweep range (`Linear<1, 64>`) so worst-case weights stay
    // measurable, with a margin matching `MaxFinalizationsPerBlock`.
    pub const MiddsMaxRemovalsPerCall: u32 = 100;
    pub const MiddsBlocksPerDay: BlockNumber = DAYS;

    // M_fast — anti-DoS, per-block reactivity. Target 100 deposits/block
    // (~17/s at 6 s/block) so a burst stuffing one block sees the multiplier
    // climb 12.5 % before the next block prices it.
    pub const MiddsFastTargetPerBlock: u32 = 100;
    pub MiddsFastAdjustmentRate: FixedU128 = FixedU128::from_rational(125, 1_000);
    pub MiddsFastMultiplierMin: FixedU128 = FixedU128::from_rational(1, 10);
    pub MiddsFastMultiplierMax: FixedU128 = FixedU128::from_u32(20);

    // M_slow — anti-flood, 7-day rolling window with day-resolution buckets.
    // Target 200 000 deposits/week (~30 K/day) — calibrage prudent V1, see
    // `docs/economics.md` decision #9.
    pub const MiddsSlowTargetPerWindow: u32 = 200_000;
    pub MiddsSlowAdjustmentRate: FixedU128 = FixedU128::from_rational(5, 100);
    pub MiddsSlowMultiplierMin: FixedU128 = FixedU128::from_rational(1, 10);
    pub MiddsSlowMultiplierMax: FixedU128 = FixedU128::from_u32(50);

    // Foundation Treasury account that receives finalized bonds and
    // multiplier premiums. Derived from a constant `PalletId` so the address
    // is deterministic and stable across runtime upgrades — never burned, by
    // tokenomics design (1 B AFT supply cap, recycled via Treasury
    // governance — `docs/economics.md` §9).
    pub const MiddsTreasuryPalletId: PalletId = PalletId(*b"af/midds");
    pub MiddsTreasuryAccount: AccountId =
        MiddsTreasuryPalletId::get().into_account_truncating();
}

impl pallet_midds::Config<pallet_midds::Instance1> for Runtime {
    type Currency = Balances;
    type RuntimeHoldReason = RuntimeHoldReason;
    type Midds = midds_types::MusicalWork;
    type ProviderOrigin = EnsureSigned<AccountId>;
    type ForceOrigin = EnsureRoot<AccountId>;
    // On-behalf flow: owner signs an off-chain `MultiSignature` payload, the
    // operator submits the runtime extrinsic. `MultiSigner::into_account()`
    // recovers the on-chain `AccountId`, matching the runtime's `Signature
    // = MultiSignature` convention from `primitives`.
    type OffchainSignature = Signature;
    type Signer = MultiSigner;
    type TreasuryAccount = MiddsTreasuryAccount;
    type DepositBase = MiddsDepositBase;
    type DepositPerByte = MiddsDepositPerByte;
    type CommitmentWindow = MiddsCommitmentWindow;
    type MaxFinalizationsPerBlock = MiddsMaxFinalizationsPerBlock;
    type MaxRemovalsPerCall = MiddsMaxRemovalsPerCall;
    type BlocksPerDay = MiddsBlocksPerDay;
    type FastTargetPerBlock = MiddsFastTargetPerBlock;
    type FastAdjustmentRate = MiddsFastAdjustmentRate;
    type FastMultiplierMin = MiddsFastMultiplierMin;
    type FastMultiplierMax = MiddsFastMultiplierMax;
    type SlowTargetPerWindow = MiddsSlowTargetPerWindow;
    type SlowAdjustmentRate = MiddsSlowAdjustmentRate;
    type SlowMultiplierMin = MiddsSlowMultiplierMin;
    type SlowMultiplierMax = MiddsSlowMultiplierMax;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = MusicalWorksBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct MusicalWorksBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_midds::BenchmarkHelper<midds_types::MusicalWork, Signature, AccountId>
    for MusicalWorksBenchmarkHelper
{
    fn bench_instance(size: u32) -> midds_types::MusicalWork {
        use frame_support::BoundedVec;
        use midds_types::{Creator, CreatorId, CreatorRole, MusicalWorkV1, WorkType};

        // Build a minimal valid MusicalWorkV1; size hint is mostly carried by
        // the title (bounded at 256 bytes), padded to approximate the requested
        // encoded length. ISWC is derived from `size` so distinct sizes yield
        // distinct payloads — required by the post-economics `PayloadHashes`
        // guard for benchmarks that queue multiple records back-to-back
        // (notably `force_remove_many`).
        let title_len = (size as usize).min(midds_types::TITLE_MAX_LEN as usize);
        let title = BoundedVec::try_from(alloc::vec![b'a'; title_len.max(1)])
            .expect("title clamped to TITLE_MAX_LEN");
        let iswc = bench_iswc_from_size(size);
        let ipi = BoundedVec::try_from(b"123456789".to_vec()).expect("9-byte IPI literal");
        let creators = BoundedVec::try_from(alloc::vec![Creator {
            role: CreatorRole::Composer,
            id: CreatorId::Ipi(ipi),
        }])
        .expect("single creator fits CREATORS_MAX");

        midds_types::MusicalWork::V1(MusicalWorkV1 {
            iswc,
            title,
            creation_year: 2025,
            instrumental: false,
            language: None,
            bpm: None,
            key: None,
            work_type: WorkType::Original,
            creators,
            classical_info: None,
            offchain_extension: None,
        })
    }

    fn create_signature(entropy: &[u8], msg: &[u8]) -> (Signature, AccountId) {
        bench_create_signature(entropy, msg)
    }
}

/// Build a structurally-valid 11-byte ISWC literal from a numeric seed —
/// `T` + 10 ASCII digits, with the seed in the low decimal positions. The
/// pallet only enforces format (charset + length), not the CISAC checksum,
/// so this synthetic ISWC is admissible for benchmarking.
#[cfg(feature = "runtime-benchmarks")]
fn bench_iswc_from_size(size: u32) -> midds_traits::Iswc {
    use frame_support::BoundedVec;
    let mut bytes = [b'0'; 11];
    bytes[0] = b'T';
    let mut n = size;
    for slot in bytes[1..].iter_mut().rev() {
        *slot = b'0' + (n % 10) as u8;
        n /= 10;
    }
    BoundedVec::try_from(bytes.to_vec()).expect("11-byte literal fits ISWC bound")
}

/// Generate a deterministic `(MultiSignature, AccountId)` pair valid for
/// `msg`, shared by every per-instance [`pallet_midds::BenchmarkHelper`]
/// (the on-behalf signature flow is identical across MIDDS kinds). Uses
/// `sp_io::crypto::sr25519_generate` so the signing key lives in the
/// benchmark keystore — no `sp-core/full_crypto` requirement, which keeps
/// the runtime build `no_std`-clean. `entropy` becomes a SecretUri
/// derivation path so distinct entropy inputs yield distinct signers.
#[cfg(feature = "runtime-benchmarks")]
fn bench_create_signature(entropy: &[u8], msg: &[u8]) -> (Signature, AccountId) {
    use sp_runtime::traits::IdentifyAccount as _;
    let path = core::str::from_utf8(entropy).unwrap_or("bench");
    let uri = alloc::format!("//{path}");
    let public = sp_io::crypto::sr25519_generate(0.into(), Some(uri.into_bytes()));
    let account: AccountId = MultiSigner::Sr25519(public).into_account();
    let sig = sp_io::crypto::sr25519_sign(0.into(), &public, msg)
        .expect("keystore available in benchmark context; qed");
    (Signature::Sr25519(sig), account)
}

// MIDDS Instance2 — `Recording` (ISRC-keyed). Same economic model as
// Instance1: V1 deliberately shares one bond/window/multiplier calibration
// across MIDDS kinds (see `../midds-sdk/docs/economics.md`), so the
// `parameter_types!` above are reused verbatim. Only the stored payload and
// the benchmark helper differ.
impl pallet_midds::Config<pallet_midds::Instance2> for Runtime {
    type Currency = Balances;
    type RuntimeHoldReason = RuntimeHoldReason;
    type Midds = midds_types::Recording;
    type ProviderOrigin = EnsureSigned<AccountId>;
    type ForceOrigin = EnsureRoot<AccountId>;
    type OffchainSignature = Signature;
    type Signer = MultiSigner;
    type TreasuryAccount = MiddsTreasuryAccount;
    type DepositBase = MiddsDepositBase;
    type DepositPerByte = MiddsDepositPerByte;
    type CommitmentWindow = MiddsCommitmentWindow;
    type MaxFinalizationsPerBlock = MiddsMaxFinalizationsPerBlock;
    type MaxRemovalsPerCall = MiddsMaxRemovalsPerCall;
    type BlocksPerDay = MiddsBlocksPerDay;
    type FastTargetPerBlock = MiddsFastTargetPerBlock;
    type FastAdjustmentRate = MiddsFastAdjustmentRate;
    type FastMultiplierMin = MiddsFastMultiplierMin;
    type FastMultiplierMax = MiddsFastMultiplierMax;
    type SlowTargetPerWindow = MiddsSlowTargetPerWindow;
    type SlowAdjustmentRate = MiddsSlowAdjustmentRate;
    type SlowMultiplierMin = MiddsSlowMultiplierMin;
    type SlowMultiplierMax = MiddsSlowMultiplierMax;
    type WeightInfo = ();
    #[cfg(feature = "runtime-benchmarks")]
    type BenchmarkHelper = RecordingsBenchmarkHelper;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct RecordingsBenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
impl pallet_midds::BenchmarkHelper<midds_types::Recording, Signature, AccountId>
    for RecordingsBenchmarkHelper
{
    fn bench_instance(size: u32) -> midds_types::Recording {
        use frame_support::BoundedVec;
        use midds_types::{PartyId, RecordingV1, WorkRef};

        // Minimal valid RecordingV1; the size hint is carried by the title
        // (bounded at TITLE_MAX_LEN), padded to approximate the requested
        // encoded length. The ISRC is derived from `size` so distinct sizes
        // yield distinct payloads — required by the post-economics
        // `PayloadHashes` guard for benchmarks that queue several records
        // back-to-back (notably `force_remove_many`).
        let title_len = (size as usize).min(midds_types::TITLE_MAX_LEN as usize);
        let title = BoundedVec::try_from(alloc::vec![b'a'; title_len.max(1)])
            .expect("title clamped to TITLE_MAX_LEN");
        let isrc = bench_isrc_from_size(size);
        let ipi = BoundedVec::try_from(b"123456789".to_vec()).expect("9-byte IPI literal");

        midds_types::Recording::V1(RecordingV1 {
            isrc,
            title,
            title_aliases: Default::default(),
            artist: PartyId::Ipi(ipi),
            // `WorkRef::Midds` has no format constraint (the referenced id
            // need not exist for a format-only on-chain check), keeping the
            // synthetic payload admissible without a companion MusicalWork.
            work: WorkRef::Midds(0),
            genres: Default::default(),
            record_year: None,
            version_type: None,
            performers: Default::default(),
            producers: Default::default(),
            duration: None,
            bpm: None,
            key: None,
            places: None,
            contributors: Default::default(),
            offchain_extension: None,
        })
    }

    fn create_signature(entropy: &[u8], msg: &[u8]) -> (Signature, AccountId) {
        bench_create_signature(entropy, msg)
    }
}

/// Build a structurally-valid 12-byte ISRC literal from a numeric seed —
/// `US` (uppercase country) + `AAA` (alphanumeric registrant) + 7 digits
/// carrying the seed in the low decimal positions. The pallet only enforces
/// format (charset + length), not the IFPI structure beyond that, so this
/// synthetic ISRC is admissible for benchmarking.
#[cfg(feature = "runtime-benchmarks")]
fn bench_isrc_from_size(size: u32) -> midds_traits::Isrc {
    use frame_support::BoundedVec;
    let mut bytes = *b"USAAA0000000";
    let mut n = size;
    for slot in bytes[5..].iter_mut().rev() {
        *slot = b'0' + (n % 10) as u8;
        n /= 10;
    }
    BoundedVec::try_from(bytes.to_vec()).expect("12-byte literal fits ISRC bound")
}

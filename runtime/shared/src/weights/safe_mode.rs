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

//! Autogenerated weights for pallet_safe_mode
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 45.0.0
//! DATE: 2025-01-16, STEPS: `50`, REPEAT: `20`, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! WORST CASE MAP SIZE: `1000000`
//! HOSTNAME: `melodie-node-1`, CPU: `<UNKNOWN>`
//! EXECUTION: , WASM-EXECUTION: Compiled, CHAIN: None, DB CACHE: 1024

// Executed Command:
// frame-omni-bencher
// v1
// benchmark
// pallet
// --runtime
// ./target/production/wbuild/melodie-runtime/melodie_runtime.compact.compressed.wasm
// --genesis-builder-preset=development
// --pallet=pallet_safe_mode
// --extrinsic=*
// --output=./runtime/shared/src/weights/safe_mode.rs
// --header=./HEADER
// --template=./.maintain/frame-weight-template.hbs

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(missing_docs)]

use polkadot_sdk::polkadot_sdk_frame as frame;
use frame::{traits::Get, deps::frame_support::weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for pallet_safe_mode.
pub trait WeightInfo {
	fn on_initialize_noop() -> Weight;
	fn on_initialize_exit() -> Weight;
	fn enter() -> Weight;
	fn force_enter() -> Weight;
	fn extend() -> Weight;
	fn force_extend() -> Weight;
	fn force_exit() -> Weight;
	fn release_deposit() -> Weight;
	fn force_release_deposit() -> Weight;
	fn force_slash_deposit() -> Weight;
}

/// Weights for pallet_safe_mode using the Allfeat node and recommended hardware.
pub struct AllfeatWeight<T>(PhantomData<T>);
impl<T: polkadot_sdk::frame_system::Config> polkadot_sdk::pallet_safe_mode::WeightInfo for AllfeatWeight<T> {
	/// Storage: `SafeMode::EnteredUntil` (r:1 w:0)
	/// Proof: `SafeMode::EnteredUntil` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	fn on_initialize_noop() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `1489`
		// Minimum execution time: 4_480_000 picoseconds.
		Weight::from_parts(4_840_000, 1489)
			.saturating_add(T::DbWeight::get().reads(1_u64))
	}
	/// Storage: `SafeMode::EnteredUntil` (r:1 w:1)
	/// Proof: `SafeMode::EnteredUntil` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	fn on_initialize_exit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `103`
		//  Estimated: `1489`
		// Minimum execution time: 12_400_000 picoseconds.
		Weight::from_parts(13_440_000, 1489)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn enter() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 0_000 picoseconds.
		Weight::from_parts(0, 0)
	}
	/// Storage: `SafeMode::EnteredUntil` (r:1 w:1)
	/// Proof: `SafeMode::EnteredUntil` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	fn force_enter() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `76`
		//  Estimated: `1489`
		// Minimum execution time: 14_720_000 picoseconds.
		Weight::from_parts(15_320_000, 1489)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	fn extend() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 0_000 picoseconds.
		Weight::from_parts(0, 0)
	}
	/// Storage: `SafeMode::EnteredUntil` (r:1 w:1)
	/// Proof: `SafeMode::EnteredUntil` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	fn force_extend() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `103`
		//  Estimated: `1489`
		// Minimum execution time: 15_760_000 picoseconds.
		Weight::from_parts(16_760_000, 1489)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: `SafeMode::EnteredUntil` (r:1 w:1)
	/// Proof: `SafeMode::EnteredUntil` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	fn force_exit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `103`
		//  Estimated: `1489`
		// Minimum execution time: 15_320_000 picoseconds.
		Weight::from_parts(17_320_000, 1489)
			.saturating_add(T::DbWeight::get().reads(1_u64))
			.saturating_add(T::DbWeight::get().writes(1_u64))
	}
	/// Storage: `SafeMode::Deposits` (r:1 w:1)
	/// Proof: `SafeMode::Deposits` (`max_values`: None, `max_size`: Some(68), added: 2543, mode: `MaxEncodedLen`)
	/// Storage: `SafeMode::EnteredUntil` (r:1 w:0)
	/// Proof: `SafeMode::EnteredUntil` (`max_values`: Some(1), `max_size`: Some(4), added: 499, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Holds` (r:1 w:1)
	/// Proof: `Balances::Holds` (`max_values`: None, `max_size`: Some(157), added: 2632, mode: `MaxEncodedLen`)
	fn release_deposit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `225`
		//  Estimated: `3622`
		// Minimum execution time: 69_561_000 picoseconds.
		Weight::from_parts(72_921_000, 3622)
			.saturating_add(T::DbWeight::get().reads(3_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: `SafeMode::Deposits` (r:1 w:1)
	/// Proof: `SafeMode::Deposits` (`max_values`: None, `max_size`: Some(68), added: 2543, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Holds` (r:1 w:1)
	/// Proof: `Balances::Holds` (`max_values`: None, `max_size`: Some(157), added: 2632, mode: `MaxEncodedLen`)
	fn force_release_deposit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `225`
		//  Estimated: `3622`
		// Minimum execution time: 66_960_000 picoseconds.
		Weight::from_parts(70_961_000, 3622)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
	/// Storage: `SafeMode::Deposits` (r:1 w:1)
	/// Proof: `SafeMode::Deposits` (`max_values`: None, `max_size`: Some(68), added: 2543, mode: `MaxEncodedLen`)
	/// Storage: `Balances::Holds` (r:1 w:1)
	/// Proof: `Balances::Holds` (`max_values`: None, `max_size`: Some(157), added: 2632, mode: `MaxEncodedLen`)
	fn force_slash_deposit() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `225`
		//  Estimated: `3622`
		// Minimum execution time: 53_920_000 picoseconds.
		Weight::from_parts(56_601_000, 3622)
			.saturating_add(T::DbWeight::get().reads(2_u64))
			.saturating_add(T::DbWeight::get().writes(2_u64))
	}
}

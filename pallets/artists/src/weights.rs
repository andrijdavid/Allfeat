
//! Autogenerated weights for `pallet_artists`
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-06-03, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// target/debug/allfeat
// benchmark
// pallet
// --pallet
// pallet-artists
// --extrinsic
// *
// --steps
// 50
// --repeat
// 20
// --output
// ./pallets/artists/src/weights.rs
// --chain
// dev
// --execution
// wasm
// --wasm-execution
// compiled

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn force_create(a: u32, b: u32, _c: u32, ) -> Weight;
}

/// Weight functions for `pallet_artists`.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: Artists ArtistStorage (r:1 w:1)
	// Storage: Assets Asset (r:1 w:1)
	// Storage: Assets Metadata (r:1 w:1)
	// Storage: Assets Account (r:1 w:1)
	fn force_create(a: u32, b: u32, _c: u32, ) -> Weight {
		(2_715_072_000 as Weight)
			// Standard Error: 1_321_000
			.saturating_add((12_223_000 as Weight).saturating_mul(a as Weight))
			// Standard Error: 1_321_000
			.saturating_add((11_930_000 as Weight).saturating_mul(b as Weight))
			.saturating_add(T::DbWeight::get().reads(4 as Weight))
			.saturating_add(T::DbWeight::get().writes(4 as Weight))
	}
}

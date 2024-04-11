// This file is part of Allfeat.

// Copyright (C) 2022-2024 Allfeat.
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

//! Allfeat CLI library.
#![warn(missing_docs)]
#![allow(clippy::type_complexity, clippy::too_many_arguments, clippy::large_enum_variant)]
#![cfg_attr(feature = "runtime-benchmarks", warn(unused_crate_dependencies))]
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
mod chain_specs;
mod cli;
mod client;
mod command;
mod eth;
mod rpc;
mod service;

fn main() -> sc_cli::Result<()> {
	command::run()
}

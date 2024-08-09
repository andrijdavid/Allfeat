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

use allfeat_primitives::Block;
use fc_mapping_sync::{EthereumBlockNotification, EthereumBlockNotificationSinks};
use sp_api::ConstructRuntimeApi;
use std::{path::PathBuf, sync::Arc, time::Duration};

use futures::prelude::*;
// Substrate
use sc_client_api::BlockchainEvents;
use sc_network_sync::SyncingService;
use sc_service::{Configuration, TaskManager};
// Frontier
use fc_rpc::EthTask;
pub use fc_rpc_core::types::{FeeHistoryCache, FeeHistoryCacheLimit, FilterPool};
pub use fc_storage::StorageOverride;

use crate::{
	apis::EthCompatRuntimeApiCollection,
	service::{FullBackend, FullClient},
};

/// Frontier DB backend type.
pub type FullFrontierBackend<C> = fc_db::Backend<Block, C>;

pub fn db_config_dir(config: &Configuration) -> PathBuf {
	config.base_path.config_dir(config.chain_spec.id())
}

/// Avalailable frontier backend types.
#[derive(Debug, Copy, Clone, Default, clap::ValueEnum)]
pub enum FrontierBackendType {
	/// Either RocksDb or ParityDb as per inherited from the global backend settings.
	#[default]
	KeyValue,
	/// Sql database with custom log indexing.
	Sql,
}

/// The ethereum-compatibility configuration used to run a node.
#[derive(Clone, Debug, clap::Parser)]
pub struct EthConfiguration {
	/// Maximum number of logs in a query.
	#[arg(long, default_value = "10000")]
	pub max_past_logs: u32,

	/// Maximum fee history cache size.
	#[arg(long, default_value = "2048")]
	pub fee_history_limit: u64,

	#[arg(long)]
	pub enable_dev_signer: bool,

	/// The dynamic-fee pallet target gas price set by block author
	#[arg(long, default_value = "1")]
	pub target_gas_price: u64,

	/// Maximum allowed gas limit will be `block.gas_limit * execute_gas_limit_multiplier`
	/// when using eth_call/eth_estimateGas.
	#[arg(long, default_value = "10")]
	pub execute_gas_limit_multiplier: u64,

	/// Size in bytes of the LRU cache for block data.
	#[arg(long, default_value = "50")]
	pub eth_log_block_cache: usize,

	/// Size in bytes of the LRU cache for transactions statuses data.
	#[arg(long, default_value = "50")]
	pub eth_statuses_cache: usize,

	/// Sets the frontier backend type (KeyValue or Sql)
	#[arg(long, value_enum, ignore_case = true, default_value_t = FrontierBackendType::default())]
	pub frontier_backend_type: FrontierBackendType,

	// Sets the SQL backend's pool size.
	#[arg(long, default_value = "100")]
	pub frontier_sql_backend_pool_size: u32,

	/// Sets the SQL backend's query timeout in number of VM ops.
	#[arg(long, default_value = "10000000")]
	pub frontier_sql_backend_num_ops_timeout: u32,

	/// Sets the SQL backend's auxiliary thread limit.
	#[arg(long, default_value = "4")]
	pub frontier_sql_backend_thread_count: u32,

	/// Sets the SQL backend's query timeout in number of VM ops.
	/// Default value is 200MB.
	#[arg(long, default_value = "209715200")]
	pub frontier_sql_backend_cache_size: u64,
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_tasks<RA>(
	task_manager: &TaskManager,
	client: Arc<FullClient<RA>>,
	backend: Arc<FullBackend>,
	frontier_backend: Arc<FullFrontierBackend<FullClient<RA>>>,
	filter_pool: Option<FilterPool>,
	storage_overrides: Arc<dyn StorageOverride<Block>>,
	fee_history_cache: FeeHistoryCache,
	fee_history_cache_limit: FeeHistoryCacheLimit,
	sync: Arc<SyncingService<Block>>,
	pubsub_notification_sinks: Arc<
		EthereumBlockNotificationSinks<EthereumBlockNotification<Block>>,
	>,
) where
	RA: ConstructRuntimeApi<Block, FullClient<RA>>,
	RA: Send + Sync + 'static,
	RA::RuntimeApi: EthCompatRuntimeApiCollection<Block>,
{
	match &*frontier_backend {
		fc_db::Backend::KeyValue(bd) => {
			task_manager.spawn_essential_handle().spawn(
				"frontier-mapping-sync-worker",
				Some("frontier"),
				fc_mapping_sync::kv::MappingSyncWorker::new(
					client.import_notification_stream(),
					Duration::new(6, 0),
					client.clone(),
					backend,
					storage_overrides.clone(),
					bd.clone(),
					3,
					0u32.into(),
					fc_mapping_sync::SyncStrategy::Normal,
					sync,
					pubsub_notification_sinks,
				)
				.for_each(|()| future::ready(())),
			);
		},
		fc_db::Backend::Sql(bd) => {
			task_manager.spawn_essential_handle().spawn_blocking(
				"frontier-mapping-sync-worker",
				Some("frontier"),
				fc_mapping_sync::sql::SyncWorker::run(
					client.clone(),
					backend,
					bd.clone(),
					client.import_notification_stream(),
					fc_mapping_sync::sql::SyncWorkerConfig {
						read_notification_timeout: Duration::from_secs(30),
						check_indexed_blocks_interval: Duration::from_secs(60),
					},
					fc_mapping_sync::SyncStrategy::Normal,
					sync,
					pubsub_notification_sinks,
				),
			);
		},
	}

	// Spawn Frontier EthFilterApi maintenance task.
	if let Some(filter_pool) = filter_pool {
		// Each filter is allowed to stay in the pool for 100 blocks.
		const FILTER_RETAIN_THRESHOLD: u64 = 100;
		task_manager.spawn_essential_handle().spawn(
			"frontier-filter-pool",
			Some("frontier"),
			EthTask::filter_pool_task(client.clone(), filter_pool, FILTER_RETAIN_THRESHOLD),
		);
	}

	// Spawn Frontier FeeHistory cache maintenance task.
	task_manager.spawn_essential_handle().spawn(
		"frontier-fee-history",
		Some("frontier"),
		EthTask::fee_history_task(
			client,
			storage_overrides,
			fee_history_cache,
			fee_history_cache_limit,
		),
	);
}

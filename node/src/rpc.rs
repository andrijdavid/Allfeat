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

// std
use std::sync::Arc;
// Allfeat
use allfeat_primitives::*;
use jsonrpsee::RpcModule;

/// Extra dependencies for GRANDPA
pub struct GrandpaDeps<BE> {
    /// Voting round info.
    pub shared_voter_state: sc_consensus_grandpa::SharedVoterState,
    /// Authority set info.
    pub shared_authority_set:
        sc_consensus_grandpa::SharedAuthoritySet<Hash, sp_runtime::traits::NumberFor<Block>>,
    /// Receives notifications about justification events from Grandpa.
    pub justification_stream: sc_consensus_grandpa::GrandpaJustificationStream<Block>,
    /// Executor to drive the subscription manager in the Grandpa RPC handler.
    pub subscription_executor: sc_rpc_spec_v2::SubscriptionTaskExecutor,
    /// Finality proof provider.
    pub finality_provider: Arc<sc_consensus_grandpa::FinalityProofProvider<BE, Block>>,
}

/// Full client dependencies
pub struct FullDeps<C, P, BE> {
    /// The client instance to use.
    pub client: Arc<C>,
    /// Transaction pool instance.
    pub pool: Arc<P>,
    /// GRANDPA specific dependencies.
    pub grandpa: GrandpaDeps<BE>,
}

/// Instantiate the base set of RPC extensions shared by every runtime.
pub fn create_full<C, P, BE>(
    deps: FullDeps<C, P, BE>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    BE: 'static + sc_client_api::backend::Backend<Block>,
    BE::State: sc_client_api::backend::StateBackend<Hashing>,
    C: 'static
        + Send
        + Sync
        + sc_client_api::AuxStore
        + sc_client_api::backend::StorageProvider<Block, BE>
        + sc_client_api::BlockchainEvents<Block>
        + sc_client_api::UsageProvider<Block>
        + sc_client_api::BlockBackend<Block>
        + sp_api::CallApiAt<Block>
        + sp_api::ProvideRuntimeApi<Block>
        + sp_blockchain::HeaderBackend<Block>
        + sp_blockchain::HeaderMetadata<Block, Error = sp_blockchain::Error>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>
        + sp_block_builder::BlockBuilder<Block>
        + substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
    P: 'static + Sync + Send + sc_transaction_pool_api::TransactionPool<Block = Block>,
{
    // polkadot-sdk
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use sc_consensus_grandpa_rpc::{Grandpa, GrandpaApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};

    let mut module = RpcModule::new(());

    let FullDeps {
        client,
        pool,
        grandpa,
    } = deps;
    let GrandpaDeps {
        shared_voter_state,
        shared_authority_set,
        justification_stream,
        subscription_executor,
        finality_provider,
    } = grandpa;

    module.merge(System::new(client.clone(), pool.clone()).into_rpc())?;
    module.merge(TransactionPayment::new(client.clone()).into_rpc())?;
    module.merge(
        Grandpa::new(
            subscription_executor,
            shared_authority_set.clone(),
            shared_voter_state,
            justification_stream,
            finality_provider,
        )
        .into_rpc(),
    )?;

    Ok(module)
}

/// Register the MIDDS RPC handlers (MusicalWorks + Recordings + Releases) on
/// top of [`create_full`].
///
/// Only runtimes hosting `pallet-midds` (e.g. Melodie) satisfy the bound; the
/// mainnet runtime keeps using the bare [`create_full`].
pub fn create_full_with_midds<C, P, BE>(
    deps: FullDeps<C, P, BE>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    BE: 'static + sc_client_api::backend::Backend<Block>,
    BE::State: sc_client_api::backend::StateBackend<Hashing>,
    C: 'static
        + Send
        + Sync
        + sc_client_api::AuxStore
        + sc_client_api::backend::StorageProvider<Block, BE>
        + sc_client_api::BlockchainEvents<Block>
        + sc_client_api::UsageProvider<Block>
        + sc_client_api::BlockBackend<Block>
        + sp_api::CallApiAt<Block>
        + sp_api::ProvideRuntimeApi<Block>
        + sp_blockchain::HeaderBackend<Block>
        + sp_blockchain::HeaderMetadata<Block, Error = sp_blockchain::Error>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>
        + sp_block_builder::BlockBuilder<Block>
        + substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>
        + midds_runtime_api::MusicalWorkApi<
            Block,
            midds_traits::Iswc,
            midds_types::MusicalWork,
            AccountId,
            Balance,
        > + midds_runtime_api::RecordingApi<
            Block,
            midds_traits::Isrc,
            midds_types::Recording,
            AccountId,
            Balance,
        > + midds_runtime_api::ReleaseApi<
            Block,
            midds_traits::Upc,
            midds_types::Release,
            AccountId,
            Balance,
        >,
    P: 'static + Sync + Send + sc_transaction_pool_api::TransactionPool<Block = Block>,
{
    // One handler per MIDDS instance. The methods are namespaced
    // (`midds_musicalWorks_*` / `midds_recordings_*` / `midds_releases_*`)
    // inside `midds-rpc`, so merging the modules into the same RPC surface
    // never collides.
    use midds_rpc::{
        MusicalWorkRpc, MusicalWorkRpcApiServer, RecordingRpc, RecordingRpcApiServer, ReleaseRpc,
        ReleaseRpcApiServer,
    };

    let client = deps.client.clone();
    let mut module = create_full(deps)?;

    module.merge(
        MusicalWorkRpc::<C, Block, midds_traits::Iswc, midds_types::MusicalWork, AccountId, Balance>::new(
            client.clone(),
        )
        .into_rpc(),
    )?;
    module.merge(
        RecordingRpc::<C, Block, midds_traits::Isrc, midds_types::Recording, AccountId, Balance>::new(
            client.clone(),
        )
        .into_rpc(),
    )?;
    module.merge(
        ReleaseRpc::<C, Block, midds_traits::Upc, midds_types::Release, AccountId, Balance>::new(
            client,
        )
        .into_rpc(),
    )?;

    Ok(module)
}

use crate::proof_rpc::db_ops::*;
use filecoin_proofs_api::{ProverId, SectorId};
use std::str::FromStr;

use entity::resource_info as ResourceInfos;
use entity::tasks as Tasks;
use entity::worker_info as WorkerInfos;
use ResourceInfos::Model as ResourceInfo;
use Tasks::Model as Task;
use WorkerInfos::Model as WorkerInfo;

use jsonrpc_core_client::transports::http;
use jsonrpc_http_server::jsonrpc_core::IoHandler;
use jsonrpsee::core::{async_trait, client::Subscription, RpcResult};
use jsonrpsee::http_client::{HttpClient, HttpClientBuilder};
use jsonrpsee::proc_macros::rpc;

use crate::resource;
use crate::utils::base64bytes::Base64Byte;
use crate::utils::{IntoAnyhow, IntoJsonRpcResult, ReveseOption};
use anyhow::anyhow;
use jsonrpc_core::ErrorCode::{InternalError, InvalidParams};
use jsonrpsee::RpcModule;
use std::sync::Arc;

#[rpc(server, client)]
pub trait ProofRpc {
    #[method(name = "Proof.SubmitC2Task")]
    async fn submit_c2_task(
        &self,
        phase1_output: Base64Byte,
        miner: String,
        prover_id: ProverId,
        sector_id: u64,
    ) -> RpcResult<String>;

    #[method(name = "Proof.GetTask")]
    async fn get_task(&self, id: String) -> RpcResult<Task>;

    #[method(name = "Proof.FetchTodo")]
    async fn fetch_todo(&self, worker_id_arg: String) -> RpcResult<Task>;

    #[method(name = "Proof.FetchUncomplete")]
    async fn fetch_uncomplte(&self, worker_id_arg: String) -> RpcResult<Vec<Task>>;

    #[method(name = "Proof.GetResourceInfo")]
    async fn get_resource_info(&self, resource_id_arg: String) -> RpcResult<Base64Byte>;

    #[method(name = "Proof.RecordProof")]
    async fn record_proof(
        &self,
        worker_id_arg: String,
        tid: String,
        proof: String,
    ) -> RpcResult<bool>;

    #[method(name = "Proof.RecordError")]
    async fn record_error(
        &self,
        worker_id_arg: String,
        tid: String,
        err_msg: String,
    ) -> RpcResult<bool>;

    #[method(name = "Proof.ListTask")]
    async fn list_task(
        &self,
        worker_id_arg: Option<String>,
        state: Option<Vec<i32>>,
    ) -> RpcResult<Vec<Task>>;
}

pub struct ProofImpl {
    resource: Arc<dyn resource::Resource + Send + Sync>,
    pool: Arc<dyn DbOp + Send + Sync>,
}

#[async_trait]
impl ProofRpcServer for ProofImpl {
    async fn submit_c2_task(
        &self,
        phase1_output: Base64Byte,
        miner: String,
        prover_id: ProverId,
        sector_id: u64,
    ) -> RpcResult<String> {
        let scp1o = serde_json::from_slice(Into::<Vec<u8>>::into(phase1_output).as_slice())
            .to_jsonrpc_result(InvalidParams)?;
        let addr =
            forest_address::Address::from_str(miner.as_str()).to_jsonrpc_result(InvalidParams)?;
        let c2_resurce = resource::C2Resource {
            prove_id: prover_id,
            sector_id: SectorId::from(sector_id),
            phase1_output: scp1o,
        };
        let resource_bytes = serde_json::to_vec(&c2_resurce).to_jsonrpc_result(InternalError)?;
        let resource_id = self
            .resource
            .store_resource_info(resource_bytes)
            .await
            .to_jsonrpc_result(InternalError)?;
        self.pool
            .add_task(addr, resource_id)
            .await
            .to_jsonrpc_result(InternalError)
    }

    async fn get_task(&self, id: String) -> RpcResult<Task> {
        self.pool.fetch(id).await.to_jsonrpc_result(InternalError)
    }

    async fn fetch_todo(&self, worker_id_arg: String) -> RpcResult<Task> {
        self.pool
            .fetch_one_todo(worker_id_arg)
            .await
            .to_jsonrpc_result(InternalError)
    }

    async fn fetch_uncomplte(&self, worker_id_arg: String) -> RpcResult<Vec<Task>> {
        self.pool
            .fetch_uncomplte(worker_id_arg)
            .await
            .to_jsonrpc_result(InternalError)
    }

    async fn get_resource_info(&self, resource_id_arg: String) -> RpcResult<Base64Byte> {
        self.resource
            .get_resource_info(resource_id_arg)
            .await
            .to_jsonrpc_result(InternalError)
    }

    async fn record_proof(
        &self,
        worker_id_arg: String,
        tid: String,
        proof: String,
    ) -> RpcResult<bool> {
        self.pool
            .record_proof(worker_id_arg, tid, proof)
            .await
            .reverse_map_err()
    }

    async fn record_error(
        &self,
        worker_id_arg: String,
        tid: String,
        err_msg: String,
    ) -> RpcResult<bool> {
        self.pool
            .record_error(worker_id_arg, tid, err_msg)
            .await
            .reverse_map_err()
    }

    async fn list_task(
        &self,
        worker_id_arg: Option<String>,
        state: Option<Vec<i32>>,
    ) -> RpcResult<Vec<Task>> {
        self.pool
            .list_task(worker_id_arg, state)
            .await
            .to_jsonrpc_result(InternalError)
    }
}

pub fn register(
    resource: Arc<dyn resource::Resource + Send + Sync>,
    pool: Arc<dyn DbOp + Send + Sync>,
) -> RpcModule<ProofImpl> {
    let proof_impl = ProofImpl { resource, pool };
    proof_impl.into_rpc()
}

pub async fn get_proxy_api(url: String) -> anyhow::Result<WrapClient> {
    HttpClientBuilder::default()
        .build(url.as_str())
        .map(|val| WrapClient { client: val })
        .anyhow()
}

pub struct WrapClient {
    client: HttpClient,
}

#[async_trait]
impl resource::Resource for WrapClient {
    async fn get_resource_info(&self, resource_id_arg: String) -> anyhow::Result<Base64Byte> {
        self.client
            .get_resource_info(resource_id_arg)
            .await
            .anyhow()
    }

    async fn store_resource_info(&self, _: Vec<u8>) -> anyhow::Result<String> {
        Err(anyhow!("not support set resource in worker"))
    }
}

#[async_trait]
impl WorkerFetch for WrapClient {
    async fn fetch_one_todo(&self, worker_id: String) -> anyhow::Result<Task> {
        self.client.fetch_todo(worker_id).await.anyhow()
    }

    async fn fetch_uncomplte(&self, worker_id_arg: String) -> anyhow::Result<Vec<Task>> {
        self.client.fetch_uncomplte(worker_id_arg).await.anyhow()
    }

    async fn record_error(
        &self,
        worker_id: String,
        tid: String,
        err_msg: String,
    ) -> Option<anyhow::Error> {
        self.client
            .record_error(worker_id, tid, err_msg)
            .await
            .err()
            .map(|e| anyhow!(e.to_string()))
    }

    async fn record_proof(
        &self,
        worker_id: String,
        tid: String,
        proof: String,
    ) -> Option<anyhow::Error> {
        self.client
            .record_proof(worker_id, tid, proof)
            .await
            .err()
            .map(|e| anyhow!(e.to_string()))
    }
}

#[async_trait]
pub trait GpuServiceRpcClient {
    async fn submit_c2_task(
        &self,
        phase1_output: Base64Byte,
        miner: String,
        prover_id: ProverId,
        sector_id: u64,
    ) -> anyhow::Result<String>;

    async fn get_task(&self, id: String) -> anyhow::Result<Task>;

    async fn fetch_todo(&self, worker_id_arg: String) -> anyhow::Result<Task>;

    async fn fetch_uncomplte(&self, worker_id_arg: String) -> anyhow::Result<Vec<Task>>;

    async fn get_resource_info(&self, resource_id_arg: String) -> anyhow::Result<Base64Byte>;

    async fn record_proof(
        &self,
        worker_id_arg: String,
        tid: String,
        proof: String,
    ) -> anyhow::Result<bool>;

    async fn record_error(
        &self,
        worker_id_arg: String,
        tid: String,
        err_msg: String,
    ) -> anyhow::Result<bool>;

    async fn list_task(
        &self,
        worker_id_arg: Option<String>,
        state: Option<Vec<i32>>,
    ) -> anyhow::Result<Vec<Task>>;
}

#[async_trait]
impl GpuServiceRpcClient for WrapClient {
    async fn submit_c2_task(
        &self,
        phase1_output: Base64Byte,
        miner: String,
        prover_id: ProverId,
        sector_id: u64,
    ) -> anyhow::Result<String> {
        self.client
            .submit_c2_task(phase1_output, miner, prover_id, sector_id)
            .await
            .anyhow()
    }

    async fn get_task(&self, id: String) -> anyhow::Result<Task> {
        self.client.get_task(id).await.anyhow()
    }

    async fn fetch_todo(&self, worker_id_arg: String) -> anyhow::Result<Task> {
        self.client.fetch_todo(worker_id_arg).await.anyhow()
    }

    async fn fetch_uncomplte(&self, worker_id_arg: String) -> anyhow::Result<Vec<Task>> {
        self.client.fetch_uncomplte(worker_id_arg).await.anyhow()
    }

    async fn get_resource_info(&self, resource_id_arg: String) -> anyhow::Result<Base64Byte> {
        self.client
            .get_resource_info(resource_id_arg)
            .await
            .anyhow()
    }

    async fn record_proof(
        &self,
        worker_id_arg: String,
        tid: String,
        proof: String,
    ) -> anyhow::Result<bool> {
        self.client
            .record_proof(worker_id_arg, tid, proof)
            .await
            .anyhow()
    }

    async fn record_error(
        &self,
        worker_id_arg: String,
        tid: String,
        err_msg: String,
    ) -> anyhow::Result<bool> {
        self.client
            .record_error(worker_id_arg, tid, err_msg)
            .await
            .anyhow()
    }

    async fn list_task(
        &self,
        worker_id_arg: Option<String>,
        state: Option<Vec<i32>>,
    ) -> anyhow::Result<Vec<Task>> {
        self.client.list_task(worker_id_arg, state).await.anyhow()
    }
}
use std::{collections::HashMap, sync::Arc, time::Duration};

use actix::prelude::*;

use super::model::{
    NamingRouteRequest, NamingRouterResponse, SyncSenderRequest, SyncSenderResponse,
    SyncSenderSetCmd,
};
use crate::common::constant::{GRPC_HEAD_KEY_CLUSTER_ID, GRPC_HEAD_KEY_SUB_NAME};
use crate::grpc::handler::NAMING_ROUTE_REQUEST;
use crate::{grpc::PayloadUtils, raft::network::factory::RaftClusterRequestSender};

pub struct ClusteSyncSender {
    //local_id:u64,
    target_id: u64,
    target_addr: Arc<String>,
    pub(crate) cluster_sender: Option<Arc<RaftClusterRequestSender>>,
    send_extend_infos: HashMap<String, String>,
}

impl ClusteSyncSender {
    pub fn new(
        local_id: u64,
        target_id: u64,
        target_addr: Arc<String>,
        cluster_sender: Option<Arc<RaftClusterRequestSender>>,
    ) -> Self {
        let mut send_extend_infos = HashMap::default();
        send_extend_infos.insert(GRPC_HEAD_KEY_CLUSTER_ID.to_owned(), local_id.to_string());
        Self {
            //local_id,
            target_id,
            target_addr,
            cluster_sender,
            send_extend_infos,
        }
    }
}

impl Actor for ClusteSyncSender {
    type Context = Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        log::info!("ClusteSyncSender started,target_id:{}", &self.target_id);
    }
}

impl Handler<SyncSenderSetCmd> for ClusteSyncSender {
    type Result = anyhow::Result<SyncSenderResponse>;

    fn handle(&mut self, msg: SyncSenderSetCmd, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            SyncSenderSetCmd::UpdateTargetAddr(target_addr) => {
                self.target_addr = target_addr;
                Ok(SyncSenderResponse::None)
            }
        }
    }
}

impl Handler<SyncSenderRequest> for ClusteSyncSender {
    type Result = ResponseActFuture<Self, anyhow::Result<SyncSenderResponse>>;

    fn handle(&mut self, msg: SyncSenderRequest, _ctx: &mut Self::Context) -> Self::Result {
        let cluster_sender = self.cluster_sender.clone();
        let target_addr = self.target_addr.clone();
        let mut send_extend_infos = self.send_extend_infos.clone();
        send_extend_infos.insert(
            GRPC_HEAD_KEY_SUB_NAME.to_string(),
            msg.0.get_sub_name().to_string(),
        );
        let fut = async move {
            let cluster_sender = match cluster_sender {
                Some(v) => v,
                None => {
                    return Err(anyhow::anyhow!(
                        "ClusteSyncSender,the cluster sender is none!"
                    ))
                }
            };
            let req = msg.0;
            let request = serde_json::to_string(&req).unwrap_or_default();
            let payload = PayloadUtils::build_full_payload(
                NAMING_ROUTE_REQUEST,
                request,
                "",
                send_extend_infos.clone(),
            );
            let resp_payload = match cluster_sender
                .send_request(target_addr.clone(), payload)
                .await
            {
                Ok(v) => v,
                Err(err) => {
                    if let NamingRouteRequest::Ping(_) = &req {
                        //ping 不重试
                        return Err(err);
                    }
                    //retry request
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    let request = serde_json::to_string(&req).unwrap_or_default();
                    let payload = PayloadUtils::build_full_payload(
                        NAMING_ROUTE_REQUEST,
                        request,
                        "",
                        send_extend_infos,
                    );
                    cluster_sender.send_request(target_addr, payload).await?
                }
            };
            let body_vec = resp_payload.body.unwrap_or_default().value;
            let _: NamingRouterResponse = serde_json::from_slice(&body_vec)?;
            Ok(SyncSenderResponse::None)
        }
        .into_actor(self)
        .map(|r, _act, _ctx| r);
        Box::pin(fut)
    }
}

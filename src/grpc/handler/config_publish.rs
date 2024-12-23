#![allow(unused_imports)]

use std::sync::Arc;

use crate::common::string_utils::StringUtils;
use crate::config::config_type::ConfigType;
use crate::config::ConfigUtils;
use crate::grpc::HandlerResult;
use crate::{
    common::appdata::AppShareData,
    config::core::{ConfigActor, ConfigAsyncCmd, ConfigCmd, ConfigKey, ConfigResult},
    grpc::{
        api_model::{BaseResponse, ConfigPublishRequest},
        nacos_proto::Payload,
        PayloadHandler, PayloadUtils,
    },
    raft::cluster::model::SetConfigReq,
};
use actix::prelude::Addr;
use async_trait::async_trait;

pub struct ConfigPublishRequestHandler {
    app_data: Arc<AppShareData>,
}

impl ConfigPublishRequestHandler {
    pub fn new(app_data: Arc<AppShareData>) -> Self {
        Self { app_data }
    }
}

#[async_trait]
impl PayloadHandler for ConfigPublishRequestHandler {
    async fn handle(
        &self,
        request_payload: crate::grpc::nacos_proto::Payload,
        _request_meta: crate::grpc::RequestMeta,
    ) -> anyhow::Result<HandlerResult> {
        let body_vec = request_payload.body.unwrap_or_default().value;
        let request: ConfigPublishRequest = serde_json::from_slice(&body_vec)?;
        let config_type = StringUtils::map_not_empty(request.get_addition_param("type").cloned())
            .map(|v| ConfigType::new_by_value(v.as_ref()).get_value());
        let desc =
            StringUtils::map_not_empty(request.get_addition_param("desc").cloned()).map(Arc::new);
        let mut req = SetConfigReq::new(
            ConfigKey::new(
                &request.data_id,
                &request.group,
                &ConfigUtils::default_tenant(request.tenant),
            ),
            request.content,
        );
        req.config_type = config_type;
        req.desc = desc;
        match self.app_data.config_route.set_config(req).await {
            Ok(_res) => {
                //let res:ConfigResult = res.unwrap();
                let mut response = BaseResponse::build_success_response();
                response.request_id = request.request_id;
                Ok(HandlerResult::success(PayloadUtils::build_payload(
                    "ConfigPublishResponse",
                    serde_json::to_string(&response)?,
                )))
            }
            Err(err) => {
                let mut response = BaseResponse::build_error_response(500u16, err.to_string());
                response.request_id = request.request_id;
                Ok(HandlerResult::success(PayloadUtils::build_payload(
                    "ErrorResponse",
                    serde_json::to_string(&response)?,
                )))
            }
        }
    }
}

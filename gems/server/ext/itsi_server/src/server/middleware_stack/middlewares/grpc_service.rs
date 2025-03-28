use super::MiddlewareLayer;
use crate::{
    ruby_types::itsi_grpc_request::ItsiGrpcRequest,
    server::{
        itsi_service::RequestContext,
        types::{HttpRequest, HttpResponse},
    },
};
use async_trait::async_trait;
use derive_more::Debug;
use either::Either;
use http::StatusCode;
use itsi_rb_helpers::{HeapVal, HeapValue};
use magnus::{block::Proc, error::Result, value::ReprValue, Symbol, Value};
use std::sync::Arc;

#[derive(Debug)]
pub struct GrpcService {
    service: Arc<HeapValue<Proc>>,
    adapter: Value, // Ruby CustomGrpcAdapter object
}

impl GrpcService {
    pub fn from_value(params: HeapVal) -> magnus::error::Result<Self> {
        let service = params.funcall::<_, _, Proc>(Symbol::new("[]"), ("service_proc",))?;
        let adapter = params.funcall::<_, _, Value>(Symbol::new("[]"), ("adapter",))?;
        Ok(GrpcService {
            service: Arc::new(service.into()),
            adapter,
        })
    }
}

#[async_trait]
impl MiddlewareLayer for GrpcService {
    async fn before(
        &self,
        req: HttpRequest,
        context: &mut RequestContext,
    ) -> Result<Either<HttpRequest, HttpResponse>> {
        // Extract gRPC method and service names from the path
        let path = req.uri().path();
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        
        if parts.len() < 2 {
            return Ok(Either::Right(HttpResponse::new(StatusCode::BAD_REQUEST)));
        }

        let service_name = parts[0].to_string();
        let method_name = parts[1].to_string();

        // Get RPC descriptor from the adapter
        let rpc_desc = self.adapter.funcall::<_, _, Option<Value>>(
            "get_rpc_desc",
            (service_name.clone(), method_name.clone()),
        )?;

        // Create gRPC request and process it
        let (grpc_req, _) = ItsiGrpcRequest::new(
            req,
            context,
            method_name,
            service_name,
            rpc_desc,
        ).await;

        grpc_req
            .process(context.ruby(), self.service.clone())
            .map_err(|e| e.into())
            .map(|_| Either::Right(HttpResponse::new(StatusCode::OK)))
    }
} 
use derive_more::Debug;
use http::{request::Parts, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Empty};
use itsi_error::from::CLIENT_CONNECTION_CLOSED;
use itsi_rb_helpers::{print_rb_backtrace, HeapValue};
use itsi_tracing::debug;
use magnus::{
    block::Proc,
    error::{ErrorType, Result as MagnusResult},
    Error,
};
use magnus::{
    value::{LazyId, ReprValue},
    Ruby, Value,
};
use std::{sync::Arc, time::Instant};
use tokio::sync::mpsc::{self};
use tracing::error;

use super::itsi_grpc_stream::ItsiGrpcStream;
use crate::server::{
    byte_frame::ByteFrame,
    itsi_service::RequestContext,
    request_job::RequestJob,
    types::{HttpRequest, HttpResponse},
};

static ID_MESSAGE: LazyId = LazyId::new("message");

#[derive(Debug)]
#[magnus::wrap(class = "Itsi::GrpcRequest", free_immediately, size)]
pub struct ItsiGrpcRequest {
    pub parts: Parts,
    pub start: Instant,
    #[debug(skip)]
    pub context: RequestContext,
    #[debug(skip)]
    pub stream: ItsiGrpcStream,
}

impl ItsiGrpcRequest {
    pub fn service_name(&self) -> MagnusResult<String> {
        let path = self.parts.uri.path();
        Ok(path.split('/').nth_back(1).unwrap().to_string())
    }

    pub fn method_name(&self) -> MagnusResult<String> {
        let path = self.parts.uri.path();
        Ok(path.split('/').nth_back(0).unwrap().to_string())
    }

    pub fn stream(&self) -> MagnusResult<ItsiGrpcStream> {
        Ok(self.stream.clone())
    }

    pub fn content_type_str(&self) -> &str {
        self.parts
            .headers
            .get("Content-Type")
            .and_then(|hv| hv.to_str().ok())
            .unwrap_or("application/x-www-form-urlencoded")
    }

    pub fn is_json(&self) -> bool {
        self.content_type_str() == "application/json"
    }

    pub fn process(self, ruby: &Ruby, app_proc: Arc<HeapValue<Proc>>) -> magnus::error::Result<()> {
        let response = self.stream.clone();
        let result = app_proc.call::<_, Value>((self,));
        if let Err(err) = result {
            Self::internal_error(ruby, response, err);
        }
        Ok(())
    }

    pub fn internal_error(_ruby: &Ruby, stream: ItsiGrpcStream, err: Error) {
        if let Some(rb_err) = err.value() {
            print_rb_backtrace(rb_err);
            stream.internal_server_error(err.to_string());
        } else {
            stream.internal_server_error(err.to_string());
        }
    }

    pub(crate) async fn process_request(
        app: Arc<HeapValue<Proc>>,
        hyper_request: HttpRequest,
        context: &RequestContext,
    ) -> itsi_error::Result<HttpResponse> {
        let (request, mut receiver) = ItsiGrpcRequest::new(hyper_request, context).await;
        let shutdown_channel = context.service.shutdown_channel.clone();
        let response_stream = request.stream.clone();
        match context
            .sender
            .send(RequestJob::ProcessGrpcRequest(request, app))
            .await
        {
            Err(err) => {
                error!("Error occurred: {}", err);
                let mut response = Response::new(BoxBody::new(Empty::new()));
                *response.status_mut() = StatusCode::BAD_REQUEST;
                Ok(response)
            }
            _ => match receiver.recv().await {
                Some(first_frame) => Ok(response_stream
                    .build_response(first_frame, receiver, shutdown_channel)
                    .await),
                None => Ok(Response::new(BoxBody::new(Empty::new()))),
            },
        }
    }
    pub fn is_connection_closed_err(ruby: &Ruby, err: &Error) -> bool {
        match err.error_type() {
            ErrorType::Jump(_) => false,
            ErrorType::Error(_, _) => false,
            ErrorType::Exception(exception) => {
                exception.is_kind_of(ruby.exception_eof_error())
                    && err
                        .value()
                        .map(|v| {
                            v.funcall::<_, _, String>(*ID_MESSAGE, ())
                                .unwrap_or("".to_string())
                                .eq(CLIENT_CONNECTION_CLOSED)
                        })
                        .unwrap_or(false)
            }
        }
    }

    pub(crate) async fn new(
        request: HttpRequest,
        context: &RequestContext,
    ) -> (ItsiGrpcRequest, mpsc::Receiver<ByteFrame>) {
        let (parts, body) = request.into_parts();
        let response_channel = mpsc::channel::<ByteFrame>(100);
        (
            Self {
                context: context.clone(),
                start: Instant::now(),
                parts,
                stream: ItsiGrpcStream::new(response_channel.0, body.into_data_stream()),
            },
            response_channel.1,
        )
    }
}

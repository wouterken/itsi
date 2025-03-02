use super::{
    bind::Bind,
    listener::{Listener, SockAddr},
};
use crate::{request::itsi_request::ItsiRequest, ITSI_SERVER};
use bytes::Bytes;
use crossbeam::channel::{Receiver, Sender};
use derive_more::Debug;
use http_body_util::{combinators::BoxBody, Empty};
use hyper::{body::Incoming, service::service_fn, Request, Response, StatusCode};
use hyper_util::{rt::TokioExecutor, server::conn::auto::Builder};
use itsi_rb_helpers::{call_with_gvl, call_without_gvl, create_ruby_thread};
use itsi_tracing::{debug, error, info};
use magnus::{
    error::Result,
    scan_args::{get_kwargs, scan_args, Args, KwArgs},
    value::Opaque,
    RHash, Ruby, Value,
};
use parking_lot::Mutex;
use std::{convert::Infallible, sync::Arc};
use tokio::runtime::Builder as RuntimeBuilder;
use tokio::task::JoinSet;

#[magnus::wrap(class = "Itsi::Server", free_immediately, size)]
#[derive(Debug)]
pub struct Server {
    #[debug(skip)]
    app: Opaque<Value>,
    #[allow(unused)]
    workers: u16,
    #[allow(unused)]
    threads: u16,
    #[allow(unused)]
    shutdown_timeout: f64,
    script_name: String,
    pub(crate) binds: Mutex<Vec<Bind>>,
}

pub enum RequestJob {
    ProcessRequest(ItsiRequest),
    Shutdown,
}

struct ThreadWorker {
    id: u16,
    app: Opaque<Value>,
    receiver: Arc<Receiver<RequestJob>>,
}

impl ThreadWorker {
    fn run(&self) -> u64 {
        debug!("Starting thread worker {}", self.id);
        let ruby = Ruby::get().unwrap();
        let server = ruby.get_inner(&ITSI_SERVER);

        call_without_gvl(|| loop {
            match self.receiver.recv() {
                Ok(RequestJob::ProcessRequest(request)) => {
                    debug!("Incoming request for worker {}", self.id);
                    match call_with_gvl(|| request.process(&ruby, server, self.app)) {
                        Ok(_) => {}
                        Err(err) => error!("Request processing failed: {}", err),
                    }
                }
                Ok(RequestJob::Shutdown) => {
                    debug!("Shutting down thread worker {}", self.id);
                    break;
                }
                Err(err) => error!("ThreadWorker {}: {}", self.id, err),
            }
        });

        0
    }
}

impl Server {
    pub fn new(args: &[Value]) -> Result<Self> {
        type OptionalArgs = (
            Option<u16>,
            Option<u16>,
            Option<f64>,
            Option<String>,
            Option<Vec<String>>,
        );

        let scan_args: Args<(), (), (), (), RHash, ()> = scan_args(args)?;
        let args: KwArgs<(Value,), OptionalArgs, ()> = get_kwargs(
            scan_args.keywords,
            &["app"],
            &[
                "workers",
                "threads",
                "shutdown_timeout",
                "script_name",
                "binds",
            ],
        )?;
        let server = Server {
            app: Opaque::from(args.required.0),
            workers: args.optional.0.unwrap_or(1),
            threads: args.optional.1.unwrap_or(1),
            shutdown_timeout: args.optional.2.unwrap_or(5.0),
            script_name: args.optional.3.unwrap_or("".to_string()),
            binds: Mutex::new(
                args.optional
                    .4
                    .unwrap_or_else(|| vec!["localhost:3000".to_string()])
                    .into_iter()
                    .map(|s| s.parse().unwrap_or_else(|_| Bind::default()))
                    .collect(),
            ),
        };
        Ok(server)
    }

    pub(crate) async fn process_request(
        hyper_request: Request<Incoming>,
        sender: Arc<Sender<RequestJob>>,
        script_name: String,
        listener: Arc<Listener>,
        addr: SockAddr,
    ) -> itsi_error::Result<Response<BoxBody<Bytes, Infallible>>> {
        let (request, receiver) =
            ItsiRequest::build_from(hyper_request, addr, script_name, listener).await;

        info!("Sending request {:?} to worker thread", request);
        match sender.send(RequestJob::ProcessRequest(request)) {
            Err(err) => {
                error!("Error occurred: {}", err);
                let mut response = Response::new(BoxBody::new(Empty::new()));
                *response.status_mut() = StatusCode::BAD_REQUEST;
                Ok(response)
            }
            _ => match receiver.await {
                Ok(response) => Ok(response.into()),
                Err(err) => {
                    error!("Recv Error occurred: {}", err);
                    let mut response = Response::new(BoxBody::new(Empty::new()));
                    *response.status_mut() = StatusCode::BAD_REQUEST;
                    Ok(response)
                }
            },
        }
    }

    pub fn start(&self) {
        let mut builder: RuntimeBuilder = RuntimeBuilder::new_current_thread();
        let runtime = builder
            .thread_name("itsi-server-accept-loop")
            .thread_stack_size(3 * 1024 * 1024)
            .enable_io()
            .enable_time()
            .build()
            .expect("Failed to build Tokio runtime");

        let (sender, receiver) = crossbeam::channel::bounded(1000);
        let receiver_ref = Arc::new(receiver);
        let sender_ref = Arc::new(sender);
        let app = self.app;

        info!(
            "Starting Itsi Server on {:?}. Threads: {}",
            self.binds.lock(),
            self.threads
        );

        call_without_gvl(|| {
            (0..=self.threads).for_each(|id| {
                let receiver = receiver_ref.clone();
                info!("Creating worker thread {}", id);
                create_ruby_thread(move || {
                    info!("Creating Ruby thread!");
                    ThreadWorker { id, app, receiver }.run();
                    0
                });
            });

            runtime.block_on(async {
                let server = Arc::new(Builder::new(TokioExecutor::new()));
                let listeners: Vec<Listener> = self
                    .binds
                    .lock()
                    .iter()
                    .cloned()
                    .map(Listener::from)
                    .collect::<Vec<_>>();

                let mut set = JoinSet::new();

                for listener in listeners {
                    let server_clone = server.clone();
                    let listener_clone = Arc::new(listener);
                    let script_name = self.script_name.clone();
                    let sender = sender_ref.clone();

                    set.spawn(async move {
                        loop {
                            let server = server_clone.clone();
                            let listener = listener_clone.clone();
                            let script_name = script_name.clone();
                            let sender = sender.clone();
                            let (stream, addr) = match listener.accept().await {
                                Ok(stream) => stream,
                                Err(e) => {
                                    error!("Failed to accept connection: {:?}", e);
                                    continue;
                                }
                            };

                            tokio::spawn(async move {
                                if let Err(e) = server
                                    .serve_connection_with_upgrades(
                                        stream,
                                        service_fn(move |hyper_request: Request<Incoming>| {
                                            Server::process_request(
                                                hyper_request,
                                                sender.clone(),
                                                script_name.clone(),
                                                listener.clone(),
                                                addr.clone(),
                                            )
                                        }),
                                    )
                                    .await
                                {
                                    info!("Closed connection due to: {:?}", e);
                                }
                            });
                        }
                    });
                }
                while let Some(_res) = set.join_next().await {}
            })
        });
    }
}

pub mod bind;
pub mod tls;
use bind::Bind;
use itsi_tracing::info;
use magnus::{
    error::Result,
    scan_args::{get_kwargs, scan_args, Args, KwArgs},
    value::ReprValue,
    RArray, RHash, Value,
};
use parking_lot::Mutex;

#[magnus::wrap(class = "Itsi::Server", free_immediately, size)]
#[derive(Debug, Default)]
pub struct Server {
    workers: u16,
    threads: u16,
    shutdown_timeout: f64,
    script_name: String,
    binds: Mutex<Vec<Bind>>,
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
        info!("Server starting {:?}", server);
        Ok(server)
    }
}

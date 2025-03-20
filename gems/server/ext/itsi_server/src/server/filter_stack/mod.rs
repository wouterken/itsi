mod filter;
mod filters;
pub use filter::Filter;
pub use filters::{
    AuthAPIKey, AuthBasic, AuthJwt, Compression, Cors, Logging, RateLimit, StaticAssets, *,
};
use itsi_rb_helpers::HeapVal;
use magnus::{
    error::Result,
    value::{LazyId, ReprValue},
    RArray, RHash, TryConvert, Value,
};
use regex::RegexSet;
use std::collections::HashMap;
use tracing::info;

use super::types::HttpRequest;

#[derive(Debug)]
pub struct FilterStack {
    pub route_set: RegexSet,
    pub stacks: HashMap<usize, Vec<Filter>>,
    pub default_stack: Vec<Filter>,
}

static ID_ROUTE: LazyId = LazyId::new("route");
static ID_SOURCE: LazyId = LazyId::new("source");
static ID_FILTERS: LazyId = LazyId::new("filters");
static ID_TYPE: LazyId = LazyId::new("type");
static ID_PARAMETERS: LazyId = LazyId::new("parameters");

impl FilterStack {
    pub fn new(routes_raw: Option<HeapVal>, default_app: HeapVal) -> Result<Self> {
        if let Some(routes_raw) = routes_raw {
            let mut stacks = HashMap::new();
            let mut routes = vec![];
            for (index, route) in RArray::from_value(*routes_raw)
                .ok_or(magnus::Error::new(
                    magnus::exception::exception(),
                    format!("Routes must be an array. Got {:?}", routes_raw),
                ))?
                .into_iter()
                .enumerate()
            {
                let route_hash: RHash = RHash::try_convert(route)?;
                let route_raw = route_hash
                    .get(*ID_ROUTE)
                    .ok_or(magnus::Error::new(
                        magnus::exception::exception(),
                        "Route is missing :route key",
                    ))?
                    .funcall::<_, _, String>(*ID_SOURCE, ())?;
                let filters =
                    RArray::from_value(route_hash.get(*ID_FILTERS).ok_or(magnus::Error::new(
                        magnus::exception::exception(),
                        "Route is missing :filters key",
                    ))?)
                    .ok_or(magnus::Error::new(
                        magnus::exception::exception(),
                        format!("filters must be an array. Got {:?}", routes_raw),
                    ))?;

                let mut filter_stack = filters
                    .into_iter()
                    .map(FilterStack::parse_filter)
                    .collect::<Result<Vec<_>>>()?;
                filter_stack.push(Filter::RackApp(RackApp::from_value(default_app.clone())?));
                routes.push(route_raw);
                stacks.insert(index, filter_stack);
            }
            Ok(Self {
                route_set: RegexSet::new(&routes).map_err(|e| {
                    magnus::Error::new(
                        magnus::exception::exception(),
                        format!("Failed to create route set: {}", e),
                    )
                })?,
                stacks,
                default_stack: vec![Filter::RackApp(RackApp::from_value(default_app)?)],
            })
        } else {
            info!("Creating rack app from default app {:?}", default_app);
            Ok(Self {
                route_set: RegexSet::empty(),
                stacks: HashMap::new(),
                default_stack: vec![Filter::RackApp(RackApp::from_value(default_app)?)],
            })
        }
    }

    pub fn stack_for(&self, request: &HttpRequest) -> &Vec<Filter> {
        if let Some(index) = self.route_set.matches(request.uri().path()).iter().next() {
            if let Some(stack) = self.stacks.get(&index) {
                stack
            } else {
                self.default_stack()
            }
        } else {
            self.default_stack()
        }
    }

    pub fn parse_filter(filter: Value) -> Result<Filter> {
        let filter_hash = RHash::from_value(filter).ok_or(magnus::Error::new(
            magnus::exception::exception(),
            format!("Filter must be a hash. Got {:?}", filter),
        ))?;
        let filter_type: String = filter_hash
            .get(*ID_TYPE)
            .ok_or(magnus::Error::new(
                magnus::exception::exception(),
                format!("Filter must have a :type key. Got {:?}", filter_hash),
            ))?
            .to_string();

        let parameters: Value = filter_hash.get(*ID_PARAMETERS).ok_or(magnus::Error::new(
            magnus::exception::exception(),
            format!("Filter must have a :parameters key. Got {:?}", filter_hash),
        ))?;

        let result = match filter_type.as_str() {
            "auth_basic" => Filter::AuthBasic(AuthBasic::from_value(parameters)?),
            "auth_jwt" => Filter::AuthJwt(Box::new(AuthJwt::from_value(parameters)?)),
            "auth_api_key" => Filter::AuthAPIKey(AuthAPIKey::from_value(parameters)?),
            "rate_limit" => Filter::RateLimit(RateLimit::from_value(parameters)?),
            "cors" => Filter::Cors(Box::new(Cors::from_value(parameters)?)),
            "static_assets" => Filter::StaticAssets(StaticAssets::from_value(parameters)?),
            "compression" => Filter::Compression(Compression::from_value(parameters)?),
            "logging" => Filter::Logging(Logging::from_value(parameters)?),
            "endpoint" => Filter::Endpoint(Endpoint::from_value(parameters)?),
            "rack_app" => Filter::RackApp(RackApp::from_value(parameters.into())?),
            "run" => Filter::RackApp(RackApp::from_value(parameters.into())?),
            _ => {
                return Err(magnus::Error::new(
                    magnus::exception::exception(),
                    format!("Unknown filter type: {}", filter_type),
                ))
            }
        };
        Ok(result)
    }

    fn default_stack(&self) -> &Vec<Filter> {
        &self.default_stack
    }

    pub(crate) fn preload(&self) -> Result<()> {
        for stack in self.stacks.values() {
            for filter in stack.iter() {
                filter.preload()?;
            }
        }
        for filter in self.default_stack.iter() {
            filter.preload()?;
        }
        Ok(())
    }
}

mod middleware;
mod middlewares;
use super::types::HttpRequest;
use itsi_rb_helpers::HeapVal;
use magnus::{error::Result, value::ReprValue, RArray, RHash, Ruby, TryConvert, Value};
pub use middleware::Middleware;
pub use middlewares::{
    AuthAPIKey, AuthBasic, AuthJwt, Compression, Cors, Logging, RateLimit, StaticAssets, *,
};
use regex::{Regex, RegexSet};
use std::collections::HashMap;

#[derive(Debug)]
pub struct MiddlewareSet {
    pub route_set: RegexSet,
    pub stacks: HashMap<usize, MiddlewareStack>,
    pub default_stack: Vec<Middleware>,
}

#[derive(Debug)]
pub struct MiddlewareStack {
    layers: Vec<Middleware>,
    methods: Option<Vec<StringMatch>>,
    protocols: Option<Vec<StringMatch>>,
    domains: Option<Vec<StringMatch>>,
    extensions: Option<Vec<StringMatch>>,
}

#[derive(Debug)]
enum StringMatch {
    Exact(String),
    Wildcard(Regex),
}

impl StringMatch {
    fn from_value(value: Value) -> Result<Self> {
        let ruby = Ruby::get().unwrap();
        if value.is_kind_of(ruby.class_regexp()) {
            let src_str = value.funcall::<_, _, String>("source", ())?;
            let regex = Regex::new(&src_str).map_err(|e| {
                magnus::Error::new(
                    magnus::exception::exception(),
                    format!("Invalid regexp: {}", e),
                )
            })?;
            Ok(StringMatch::Wildcard(regex))
        } else {
            Ok(StringMatch::Exact(value.to_string()))
        }
    }

    fn matches(&self, value: &str) -> bool {
        match self {
            StringMatch::Exact(s) => s.eq_ignore_ascii_case(value),
            StringMatch::Wildcard(re) => re.is_match(value),
        }
    }
}

impl MiddlewareStack {
    pub fn matches(&self, request: &HttpRequest) -> bool {
        if let Some(methods) = &self.methods {
            let method = request.method().as_str();
            if !methods.iter().any(|m| m.matches(method)) {
                return false;
            }
        }

        if let (Some(protocols), Some(protocol)) = (&self.protocols, request.uri().scheme()) {
            if !protocols.iter().any(|p| p.matches(protocol.as_str())) {
                return false;
            }
        }

        if let (Some(domains), Some(domain)) = (&self.domains, request.uri().host()) {
            if !domains.iter().any(|d| d.matches(domain)) {
                return false;
            }
        }

        if let Some(extensions) = &self.extensions {
            let extension = request.uri().path().split('.').next_back().unwrap_or("");
            if !extensions.iter().any(|e| e.matches(extension)) {
                return false;
            }
        }

        true
    }
}

impl MiddlewareSet {
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
                    .get("route")
                    .ok_or(magnus::Error::new(
                        magnus::exception::exception(),
                        "Route is missing :route key",
                    ))?
                    .funcall::<_, _, String>("source", ())?;
                let middleware =
                    RArray::from_value(route_hash.get("middleware").ok_or(magnus::Error::new(
                        magnus::exception::exception(),
                        "Route is missing middleware key",
                    ))?)
                    .ok_or(magnus::Error::new(
                        magnus::exception::exception(),
                        format!("middleware must be an array. Got {:?}", routes_raw),
                    ))?;

                let mut layers = middleware
                    .into_iter()
                    .map(MiddlewareSet::parse_middleware)
                    .collect::<Result<Vec<_>>>()?;
                layers.push(Middleware::RubyApp(RubyApp::from_value(
                    default_app.clone(),
                )?));
                routes.push(route_raw);
                layers.sort();
                stacks.insert(
                    index,
                    MiddlewareStack {
                        layers,
                        methods: extract_optional_match_array(route_hash, "methods")?,
                        protocols: extract_optional_match_array(route_hash, "protocols")?,
                        domains: extract_optional_match_array(route_hash, "domains")?,
                        extensions: extract_optional_match_array(route_hash, "extensions")?,
                    },
                );
            }
            Ok(Self {
                route_set: RegexSet::new(&routes).map_err(|e| {
                    magnus::Error::new(
                        magnus::exception::exception(),
                        format!("Failed to create route set: {}", e),
                    )
                })?,
                stacks,
                default_stack: vec![Middleware::RubyApp(RubyApp::from_value(default_app)?)],
            })
        } else {
            Ok(Self {
                route_set: RegexSet::empty(),
                stacks: HashMap::new(),
                default_stack: vec![Middleware::RubyApp(RubyApp::from_value(default_app)?)],
            })
        }
    }

    pub fn stack_for(&self, request: &HttpRequest) -> &Vec<Middleware> {
        let binding = self.route_set.matches(request.uri().path());
        let matches = binding.iter();
        for index in matches {
            if let Some(stack) = self.stacks.get(&index) {
                if stack.matches(request) {
                    return &stack.layers;
                }
            }
        }
        self.default_stack()
    }

    pub fn parse_middleware(middleware: Value) -> Result<Middleware> {
        let middleware_hash = RHash::from_value(middleware).ok_or(magnus::Error::new(
            magnus::exception::exception(),
            format!("Filter must be a hash. Got {:?}", middleware),
        ))?;
        let middleware_type: String = middleware_hash
            .get("type")
            .ok_or(magnus::Error::new(
                magnus::exception::exception(),
                format!("Filter must have a :type key. Got {:?}", middleware_hash),
            ))?
            .to_string();

        let parameters: Value = middleware_hash.get("parameters").ok_or(magnus::Error::new(
            magnus::exception::exception(),
            format!(
                "Filter must have a :parameters key. Got {:?}",
                middleware_hash
            ),
        ))?;

        let result = match middleware_type.as_str() {
            "auth_basic" => Middleware::AuthBasic(AuthBasic::from_value(parameters)?),
            "auth_jwt" => Middleware::AuthJwt(Box::new(AuthJwt::from_value(parameters)?)),
            "auth_api_key" => Middleware::AuthAPIKey(AuthAPIKey::from_value(parameters)?),
            "rate_limit" => Middleware::RateLimit(RateLimit::from_value(parameters)?),
            "cors" => Middleware::Cors(Box::new(Cors::from_value(parameters)?)),
            "static_assets" => Middleware::StaticAssets(StaticAssets::from_value(parameters)?),
            "compression" => Middleware::Compression(Compression::from_value(parameters)?),
            "logging" => Middleware::Logging(Logging::from_value(parameters)?),
            "endpoint" => Middleware::Endpoint(Endpoint::from_value(parameters)?),
            "app" => Middleware::RubyApp(RubyApp::from_value(parameters.into())?),
            _ => {
                return Err(magnus::Error::new(
                    magnus::exception::exception(),
                    format!("Unknown filter type: {}", middleware_type),
                ))
            }
        };
        Ok(result)
    }

    fn default_stack(&self) -> &Vec<Middleware> {
        &self.default_stack
    }
}

fn extract_optional_match_array(route_hash: RHash, arg: &str) -> Result<Option<Vec<StringMatch>>> {
    let rarray = route_hash.aref::<_, Option<RArray>>(arg)?;
    if let Some(array) = rarray {
        Ok(Some(
            array
                .into_iter()
                .map(StringMatch::from_value)
                .collect::<Result<Vec<StringMatch>>>()?,
        ))
    } else {
        Ok(None)
    }
}

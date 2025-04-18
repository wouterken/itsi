mod middleware;
mod middlewares;
use http::header::{ACCEPT, CONTENT_TYPE, HOST};
use itsi_rb_helpers::HeapVal;
use magnus::{
    error::Result, rb_sys::AsRawValue, value::ReprValue, RArray, RHash, Ruby, TryConvert, Value,
};
pub use middleware::Middleware;
pub use middlewares::*;
use regex::{Regex, RegexSet};
use std::{
    collections::{hash_map::Entry::Vacant, HashMap},
    sync::Arc,
};
use tracing::debug;

use super::http_message_types::HttpRequest;

#[derive(Debug)]
pub struct MiddlewareSet {
    pub route_set: RegexSet,
    pub patterns: Vec<Arc<Regex>>,
    pub stacks: HashMap<usize, MiddlewareStack>,
    unique_middlewares: HashMap<u64, Middleware>,
}

#[derive(Debug)]
pub struct MiddlewareStack {
    layers: Vec<Middleware>,
    methods: Option<Vec<StringMatch>>,
    protocols: Option<Vec<StringMatch>>,
    hosts: Option<Vec<StringMatch>>,
    extensions: Option<Vec<StringMatch>>,
    ports: Option<Vec<StringMatch>>,
    content_types: Option<Vec<StringMatch>>,
    accepts: Option<Vec<StringMatch>>,
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
                    magnus::exception::standard_error(),
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

        if let (Some(hosts), Some(host)) = (&self.hosts, request.headers().get(HOST)) {
            if let Ok(host) = host.to_str() {
                if !hosts.iter().any(|d| d.matches(host)) {
                    return false;
                }
            }
        }

        if let (Some(ports), Some(port)) = (&self.ports, request.uri().port()) {
            if !ports.iter().any(|d| d.matches(port.as_str())) {
                return false;
            }
        }

        if let Some(extensions) = &self.extensions {
            let path = request.uri().path();
            let segment = path.split('/').next_back().unwrap_or("");
            let extension = segment.split('.').next_back().unwrap_or("");
            let extension = if segment != extension { extension } else { "" };
            if !extensions.iter().any(|e| e.matches(extension)) {
                return false;
            }
        }

        if let Some(content_types) = &self.content_types {
            if let Some(content_type) = request.headers().get(CONTENT_TYPE) {
                if !content_types
                    .iter()
                    .any(|ct| ct.matches(content_type.to_str().unwrap_or("")))
                {
                    return false;
                }
            }
        }

        if let Some(accepts) = &self.accepts {
            if let Some(accept) = request.headers().get(ACCEPT) {
                if !accepts
                    .iter()
                    .any(|a| a.matches(accept.to_str().unwrap_or("")))
                {
                    return false;
                }
            }
        }

        true
    }
}

impl MiddlewareSet {
    pub fn new(routes_raw: Option<HeapVal>) -> Result<Self> {
        let mut unique_middlewares = HashMap::new();
        if let Some(routes_raw) = routes_raw {
            let mut stacks = HashMap::new();
            let mut routes = vec![];
            for (index, route) in RArray::from_value(*routes_raw)
                .ok_or(magnus::Error::new(
                    magnus::exception::standard_error(),
                    format!("Routes must be an array. Got {:?}", routes_raw),
                ))?
                .into_iter()
                .enumerate()
            {
                let route_hash: RHash = RHash::try_convert(route)?;
                let route_raw = route_hash
                    .get("route")
                    .ok_or(magnus::Error::new(
                        magnus::exception::standard_error(),
                        "Route is missing :route key",
                    ))?
                    .funcall::<_, _, String>("source", ())?;

                let middleware =
                    RHash::from_value(route_hash.get("middleware").ok_or(magnus::Error::new(
                        magnus::exception::standard_error(),
                        "Route is missing middleware key",
                    ))?)
                    .ok_or(magnus::Error::new(
                        magnus::exception::standard_error(),
                        format!("middleware must be a hash. Got {:?}", routes_raw),
                    ))?;

                let ruby = Ruby::get().unwrap();
                let mut layers = middleware
                    .enumeratorize("each", ())
                    .flat_map(|pair| {
                        let pair = RArray::from_value(pair.unwrap()).unwrap();
                        let middleware_type: String = pair.entry(0).unwrap();
                        let value: Value = pair.entry(1).unwrap();
                        let middleware_values: Vec<Value> = if value.is_kind_of(ruby.class_array())
                        {
                            RArray::from_value(value)
                                .ok_or_else(|| {
                                    magnus::Error::new(
                                        magnus::exception::type_error(),
                                        "Expected array",
                                    )
                                })
                                .unwrap()
                                .into_iter()
                                .collect()
                        } else {
                            vec![value]
                        };
                        middleware_values
                            .into_iter()
                            .map(|value| {
                                let middleware =
                                    if let Vacant(e) = unique_middlewares.entry(value.as_raw()) {
                                        let middleware = MiddlewareSet::parse_middleware(
                                            middleware_type.clone(),
                                            value,
                                        );
                                        if let Ok(middleware) = middleware.as_ref() {
                                            e.insert(middleware.clone());
                                        };
                                        middleware
                                    } else {
                                        Ok(unique_middlewares.get(&value.as_raw()).unwrap().clone())
                                    };
                                middleware
                            })
                            .collect::<Vec<Result<Middleware>>>()
                    })
                    .collect::<Result<Vec<_>>>()?;
                routes.push(route_raw);
                layers.sort();
                stacks.insert(
                    index,
                    MiddlewareStack {
                        layers,
                        methods: extract_optional_match_array(route_hash, "methods")?,
                        protocols: extract_optional_match_array(route_hash, "protocols")?,
                        hosts: extract_optional_match_array(route_hash, "hosts")?,
                        extensions: extract_optional_match_array(route_hash, "extensions")?,
                        ports: extract_optional_match_array(route_hash, "ports")?,
                        content_types: extract_optional_match_array(route_hash, "content_types")?,
                        accepts: extract_optional_match_array(route_hash, "accepts")?,
                    },
                );
            }
            Ok(Self {
                route_set: RegexSet::new(&routes).map_err(|e| {
                    magnus::Error::new(
                        magnus::exception::standard_error(),
                        format!("Failed to create route set: {}", e),
                    )
                })?,
                unique_middlewares,
                patterns: routes
                    .into_iter()
                    .map(|r| Regex::new(&r))
                    .collect::<std::result::Result<Vec<Regex>, regex::Error>>()
                    .map_err(|e| {
                        magnus::Error::new(
                            magnus::exception::standard_error(),
                            format!("Failed to create route set: {}", e),
                        )
                    })?
                    .into_iter()
                    .map(Arc::new)
                    .collect(),
                stacks,
            })
        } else {
            Err(magnus::Error::new(
                magnus::exception::standard_error(),
                "Failed to create middleware stack",
            ))
        }
    }

    pub fn stack_for(
        &self,
        request: &HttpRequest,
    ) -> Result<(&Vec<Middleware>, Option<Arc<Regex>>)> {
        let binding = self.route_set.matches(request.uri().path());
        let matches = binding.iter();

        debug!(
            "Matching request URI {:?} against self.route_set: {:?}",
            request.uri().path(),
            self.route_set
        );
        for index in matches {
            let matching_pattern = self.patterns.get(index).cloned();
            if let Some(stack) = self.stacks.get(&index) {
                if stack.matches(request) {
                    return Ok((&stack.layers, matching_pattern));
                }
            }
        }
        debug!(
            "Failed to match request URI {:?} to self.route_set: {:?}",
            request.uri().path(),
            self.route_set
        );
        Err(magnus::Error::new(
            magnus::exception::standard_error(),
            format!(
                "No matching middleware stack found for request: {:?}",
                request
            ),
        ))
    }

    pub fn parse_middleware(middleware_type: String, parameters: Value) -> Result<Middleware> {
        let mw_type = middleware_type.clone();

        debug!(target: "itsi_server::middleware_stack", "Parsing middleware: {} from {}", mw_type, parameters);
        let result = (move || -> Result<Middleware> {
            match mw_type.as_str() {
                "allow_list" => Ok(Middleware::AllowList(AllowList::from_value(parameters)?)),
                "auth_basic" => Ok(Middleware::AuthBasic(AuthBasic::from_value(parameters)?)),
                "auth_jwt" => Ok(Middleware::AuthJwt(AuthJwt::from_value(parameters)?)),
                "auth_api_key" => Ok(Middleware::AuthAPIKey(AuthAPIKey::from_value(parameters)?)),
                "cache_control" => Ok(Middleware::CacheControl(CacheControl::from_value(
                    parameters,
                )?)),
                "deny_list" => Ok(Middleware::DenyList(DenyList::from_value(parameters)?)),
                "etag" => Ok(Middleware::ETag(ETag::from_value(parameters)?)),
                "csp" => Ok(Middleware::Csp(Csp::from_value(parameters)?)),
                "intrusion_protection" => Ok({
                    Middleware::IntrusionProtection(IntrusionProtection::from_value(parameters)?)
                }),
                "max_body" => Ok(Middleware::MaxBody(MaxBody::from_value(parameters)?)),
                "rate_limit" => Ok(Middleware::RateLimit(RateLimit::from_value(parameters)?)),
                "cors" => Ok(Middleware::Cors(Cors::from_value(parameters)?)),
                "request_headers" => Ok(Middleware::RequestHeaders(RequestHeaders::from_value(
                    parameters,
                )?)),
                "response_headers" => Ok(Middleware::ResponseHeaders(ResponseHeaders::from_value(
                    parameters,
                )?)),
                "static_assets" => Ok(Middleware::StaticAssets(StaticAssets::from_value(
                    parameters,
                )?)),
                "static_response" => Ok(Middleware::StaticResponse(StaticResponse::from_value(
                    parameters,
                )?)),
                "compress" => Ok(Middleware::Compression(Compression::from_value(
                    parameters,
                )?)),
                "log_requests" => Ok(Middleware::LogRequests(LogRequests::from_value(
                    parameters,
                )?)),
                "redirect" => Ok(Middleware::Redirect(Redirect::from_value(parameters)?)),
                "app" => Ok(Middleware::RubyApp(RubyApp::from_value(parameters.into())?)),
                "proxy" => Ok(Middleware::Proxy(Proxy::from_value(parameters)?)),
                _ => Err(magnus::Error::new(
                    magnus::exception::standard_error(),
                    format!("Unknown filter type: {}", mw_type),
                )),
            }
        })();

        debug!(target: "itsi_server::middleware_stack", "Stack layer init result {:?}", result);

        match result {
            Ok(result) => Ok(result),
            Err(err) => Err(magnus::Error::new(
                magnus::exception::standard_error(),
                format!(
                    "Failed to instantiate middleware of type {}, due to {}",
                    middleware_type, err
                ),
            )),
        }
    }

    pub async fn initialize_layers(&self) -> Result<()> {
        for middleware in self.unique_middlewares.values() {
            middleware.initialize().await?;
        }
        Ok(())
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

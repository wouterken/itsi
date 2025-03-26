use super::{
    server::static_file_server::{ErrorResponse, NotFoundBehavior, ServeRange, StaticFileServer, StaticFileServerConfig},
    FromValue, MiddlewareLayer,
};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
// ... existing code ...
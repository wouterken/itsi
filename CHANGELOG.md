## [0.2.17] - Unreleased
- Add 5 threads by default in rack/handler
- Reserve header size ahead of time in rack interface
- Avoid intermediate array allocation when populating Rack env headers.
- Rewrite synchronous thread worker to avoid excessive GVL acquisition
- Revert to default write_ev behaviour for http1
- Switch to service_fn from service struct to avoid one additional pinned future
- Worker pinning accepts ruby workers too
- Fixed ordering incomaptibility in etag forwarding from static file server
- Added embedded benchmark suite

## [0.2.16] - 2025-05-02
- Optimized static error responses
- Optimized rate limit middleware
- Made default static serve command use more efficient defaults
- Reduced cloning in main accept-loop
- Fixed ability to set nodelay to false.
- Added send_buffer_size option.
- Worker pinning accepts ruby workers too
- Fixed ordering incomaptibility in etag forwarding from static file server

## [0.2.14] - 2025-04-30
- Support new-line separated headers for Rack 2 backward compatibility.

## [0.2.12] - 2025-04-29
- Max Rust edition is now "2021"
- Removed invalid rbs files causing RI doc generation failure
- Fixed header clobbering in Rack
- Added new `ruby_thread_request_backlog_size` option

## [0.2.3] - 2025-04-22

- Public release!
- https://itsi.fyi

## [0.1.0] - 2025-02-28

- Initial release

auto_reload_config! # Auto-reload the server configuration each time it changes.

bind "http://0.0.0.0:8080"


# Admin area.
# Credentials inline for simplicity.
# Use `itsi passfile` to better manage credential files.
location "admin*" do
  auth_basic \
    realm: "Admin Area",
    # admin:admin
    credential_pairs: {
      "admin": "$5$rounds=1000$g/UE8n2JbHo0fnBU$FK2NZYTVzWrMBFfadoWeETfVZkPcegxjE23IJYjkUI1"
    }
end

static_assets \
  auto_index: true,
  # We restrict serving to *just* `txt`, `png`, and `csv` files.
  # HTML file serving is implicit (due to `auto_index`) Otherwise this must be explicit.
  allowed_extensions: %w[txt png csv]

# Add a rate limit
rate_limit \
  requests: 3,
  seconds: 5,
  key: "address",
  store_config: "in_memory",
  error_response: "too_many_requests"

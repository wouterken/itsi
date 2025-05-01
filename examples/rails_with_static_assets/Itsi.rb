auto_reload_config! # Auto-reload the server configuration each time it changes.

# Everything under /assets/* gets intercepted by our static file server *before* it gets to Rails.
# We use not_found_behaviour: "fallthrough" to fall through to Rails static file serving, if
# the file isn't found.
location "assets*" do
  etag type: "strong", algorithm: "sha256"
  static_assets root_dir: "./public", not_found_behavior: "fallthrough", relative_path: false, headers: {
    "cache-control" => "public, max-age=5"
  }
end

rackup_file "config.ru"

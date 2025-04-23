auto_reload_config! # Auto-reload the server configuration each time it changes.

# Everything under /assets/* gets intercepted by our static file server *before* it gets to Rails.
# We use not_found_behaviour: "fallthrough" to fall through to Rails static file serving, if
# the file isn't found.
location "assets*" do
  static_assets root_dir: "./public", not_found_behavior: "fallthrough", relative_path: false
end

rackup_file "config.ru"

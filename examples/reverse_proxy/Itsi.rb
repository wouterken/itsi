# Make sure to run bundle install and  bundle exec rails db:migrate before running this test.
#
# You can visit "/" to see an index to pick between the Rails and Sinatra app.
# Go to rails/articles

auto_reload_config! # Auto-reload the server configuration each time it changes.

# Example of how we can use string rewrite modifiers
location "/rails*" do
  proxy to: "http://localhost:4000{path_and_query|strip_prefix:/rails}"
end
# We assume all assets are hosted by Rails in this simple example
location "/assets*" do
  proxy to: "http://localhost:4000{path_and_query}"
end

# route sub-path requests to top level of sinatra app
location "/sinatra*" do
  proxy to: "http://localhost:6000{path_and_query|strip_prefix:/sinatra}"
end
location "/api/status" do
  proxy to: "http://localhost:6000{path_and_query}"
end

# Fallthrough
static_assets \
  root_dir: "./",
  not_found_behavior: {error: "not_found"}

# Make sure to run `bundle install` and `bundle exec rails db:migrate` before running this test.
#
# You can visit "/" to see an index file where you can pick between the Rails and Sinatra app.
# Go to rails/articles

auto_reload_config! # Auto-reload the server configuration each time it changes.

log_level :info

# First we mount our Rails app.
# When hosting a Rails app under a sub-directory,
# we also need to set our `config.relative_url_root`
# E.g.
#
# Inside application.rb
#
# config.relative_url_root = "/rails"
#
location '/' do
  location '/rails*' do
    location '/articles*' do
      # Example of how we can selectively apply middleware to separate routes.
      rate_limit \
        requests: 1,
        seconds: 10,
        key: 'address',
        store_config: 'in_memory',
        error_response: 'too_many_requests'
    end

    rackup_file 'rails_subapp/config.ru'
  end

  # Next we mount our Sinatra app.
  #
  location '/sinatra*' do
    rackup_file 'sinatra_subapp/config.ru'
  end

  # Create a fallback and root HTML page.
  static_assets root_dir: './', not_found_behavior: 'fallthrough'
end

static_assets root_dir: './', not_found_behavior: { error: 'not_found' }

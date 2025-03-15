require_relative "boot"

require "rails/all"

# Require the gems listed in Gemfile, including any gems
# you've limited to :test, :development, or :production.
Bundler.require(*Rails.groups)

module ItsiSandboxRails
  class Application < Rails::Application
    # Initialize configuration defaults for originally generated Rails version.
    config.load_defaults 8.0

    # Please, add to the `ignore` list any other `lib` subdirectories that do
    # not contain `.rb` files, or that should not be reloaded or eager loaded.
    # Common ones are `templates`, `generators`, or `middleware`, for example.
    config.autoload_lib(ignore: %w[assets tasks])

    # Test impact of certain middleware combinations on performance
    # here.
    [
      # ActionDispatch::HostAuthorization,
      # Rack::Sendfile,
      # ActionDispatch::Static,
      # ActionDispatch::Executor,
      # ActionDispatch::ServerTiming,
      # Rack::Runtime,
      # Rack::MethodOverride,
      # ActionDispatch::RequestId,
      # ActionDispatch::RemoteIp,
      # Rails::Rack::Logger,
      # ActionDispatch::ShowExceptions,
      # ActionDispatch::DebugExceptions,
      # ActionDispatch::ActionableExceptions,
      # ActionDispatch::Reloader,
      # ActionDispatch::Callbacks,
      # ActiveRecord::Migration::CheckPending,
      # ActionDispatch::Cookies,
      # ActionDispatch::Session::CookieStore,
      # ActionDispatch::Flash,
      # ActionDispatch::ContentSecurityPolicy::Middleware,
      # ActionDispatch::PermissionsPolicy::Middleware,
      # Rack::Head,
      # Rack::ConditionalGet,
      # Rack::ETag,
      # Rack::TempfileReaper
    ].each do |mw|
      config.middleware.delete(mw)
    end



    # Configuration for the application, engines, and railties goes here.
    #
    # These settings can be overridden in specific environments using the files
    # in config/environments, which are processed later.
    #
    # config.time_zone = "Central Time (US & Canada)"
    # config.eager_load_paths << Rails.root.join("extras")
  end
end

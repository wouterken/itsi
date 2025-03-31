# When you Run Itsi without a Rack app,
# we start a tiny little echo server, just so you can see it in action.
DEFAULT_INDEX = IO.read("#{__dir__}/index.html").freeze
DEFAULT_BINDS = ["http://0.0.0.0:3000"].freeze
DEFAULT_APP = lambda {
  require "json"
  Itsi.log_warn "No config.ru or Itsi.rb app detected. Running default app."
  Itsi::Server::RackInterface.for(lambda do |env|
    headers, body = \
      if env["itsi.response"].json?
        [
          { "Content-Type" => "application/json" },
          [{ "message" => "You're running on Itsi!", "rack_env" => env,
              "version" => Itsi::Server::VERSION }.to_json]
        ]
      else
        [
          { "Content-Type" => "text/html" },
          [
            format(
              DEFAULT_INDEX,
              REQUEST_METHOD: env["REQUEST_METHOD"],
              PATH_INFO: env["PATH_INFO"],
              SERVER_NAME: env["SERVER_NAME"],
              SERVER_PORT: env["SERVER_PORT"],
              REMOTE_ADDR: env["REMOTE_ADDR"],
              HTTP_USER_AGENT: env["HTTP_USER_AGENT"]
            )
          ]
        ]
      end
    [200, headers, body]
  end)
}

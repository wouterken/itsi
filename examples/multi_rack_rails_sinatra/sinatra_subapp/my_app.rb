require 'sinatra'
require 'json'
require 'time'

class MyApp < Sinatra::Base
  enable :inline_templates

  # Root: random greeting + time in a styled HTML template
  get '/' do
    greetings = [
      "👋 Hello, world!",
      "¡Hola, mundo!",
      "Bonjour le monde!",
      "こんにちは世界！",
      "Hallo Welt!",
      "Ciao mondo!"
    ]
    @greeting = greetings.sample
    @time     = Time.now.strftime("%Y‑%m‑%d %H:%M:%S %Z")
    erb :index
  end

  # JSON endpoint
  get '/api/status' do
    content_type :json
    {
      status:    'ok',
      timestamp: Time.now.iso8601,
      greeting:  @greeting || "Hello"
    }.to_json
  end

  # 404
  not_found do
    status 404
    erb :not_found
  end

  # 500
  error do
    erb :error, locals: { message: env['sinatra.error'].message }
  end
end


__END__

@@ layout
<!DOCTYPE html>
<html>
  <head>
    <meta charset="utf-8">
    <title>MyApp</title>
    <style>
      body {
        font-family: -apple-system, BlinkMacSystemFont, sans-serif;
        background: #eef2f5;
        color: #333;
        display: flex; justify-content: center; align-items: center;
        height: 100vh; margin: 0;
      }
      .card {
        background: #fff;
        padding: 2rem;
        border-radius: 0.5rem;
        box-shadow: 0 4px 12px rgba(0,0,0,0.1);
        text-align: center;
      }
      h1 { margin: 0 0 1rem; font-size: 2rem; }
      p { margin: 0.5rem 0; }
      a { color: #007acc; text-decoration: none; }
      a:hover { text-decoration: underline; }
    </style>
  </head>
  <body>
    <div class="card">
      <%= yield %>
      <p><a href="/sinatra/api/status">View JSON status</a></p>
    </div>
  </body>
</html>

@@ index
<h1><%= @greeting %></h1>
<p>Current server time:</p>
<p><strong><%= @time %></strong></p>

@@ not_found
<h1>404 – Page Not Found</h1>
<p>Sorry, we couldn’t find what you’re looking for.</p>
<p><a href="/sinatra">Back home</a></p>

@@ error
<h1>500 – Something Went Wrong</h1>
<p><%= message %></p>
<p><a href="/sinatra">Try again</a></p>

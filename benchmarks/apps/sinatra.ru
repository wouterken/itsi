require 'sinatra/base'

class MyApp < Sinatra::Base
  get '/get' do
    "Hello, world!"
  end

  post '/post' do
    "You posted: #{request.body.read}"
  end
end

run MyApp

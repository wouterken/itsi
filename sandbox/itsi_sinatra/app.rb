require 'sinatra'
require 'itsi/server'

set :server, "itsi"

get '/' do
  'Hello world!'
end

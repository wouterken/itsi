require "json"

auto_reload_config! # Auto-reload the server configuration each time it changes.

include "schemas"
include "controllers"
include "../helpers/datastore"

location '/users' do
  controller UserController.new
  get    '',       :index
  get    '/:id',   :show
  post   '',       :create
  put    '/:id',   :update
  delete '/:id',   :destroy
end

location '/posts' do
  controller PostController.new
  get    '',       :index
  get    '/:id',   :show
  post   '',       :create
  put    '/:id',   :update
  delete '/:id',   :destroy
end

endpoint do |req|
  req.ok json: {
    message: "Welcome to the Users and Posts API",
    routes: [
      {route: "/users(/:id)?", methods: %w[post delete get]},
      {route: "/posts(/:id)?", methods: %w[post delete get]},
    ]
  }
end

require "json"
include "../helpers/datastore"
auto_reload_config! # Auto-reload the server configuration each time it changes.

# Schemas
UserInputSchema = {
  _required: %i[name email],
  name:     String,
  email:    String
}

UserResponseSchema = {
  _required: %i[id name email],
  id:        Integer,
  name:      String,
  email:     String
}

PostInputSchema = {
  _required: %i[title body],
  title:     String,
  body:      String
}

PostResponseSchema = {
  _required: %i[id title body],
  id:        Integer,
  title:     String,
  body:      String
}

# Endpoints

# List all users
get "/users" do |req|
  users = User.all.map(&:to_h)
  req.ok json: users, as: Array[UserResponseSchema]
end

# Get a single user
get "/users/:id" do |req|
  user = User.find(req.url_params[:id])
  if user
    req.ok json: user.to_h, as: UserResponseSchema
  else
    req.not_found "User not found"
  end
end

# Create a new user
post "/users" do |req, params: UserInputSchema|
  user = User.create!(params)
  req.created json: user.to_h, as: UserResponseSchema
end

# Update an existing user
put "/users/:id" do |req, params: UserInputSchema|
  user = User.find(req.url_params[:id])
  if user
    user.update!(params)
    req.ok json: user.to_h, as: UserResponseSchema
  else
    req.not_found "User not found"
  end
end

# Delete a user
delete "/users/:id" do |req|
  if User.delete(req.url_params[:id])
    req.ok json: { message: "Deleted" }
  else
    req.not_found "User not found"
  end
end

# List all posts
get "/posts" do |req|
  posts = Post.all.map(&:to_h)
  req.ok json: posts, as: Array[PostResponseSchema]
end

# Get a single post
get "/posts/:id" do |req|
  post = Post.find(req.url_params[:id])
  if post
    req.ok json: post.to_h, as: PostResponseSchema
  else
    req.not_found "Post not found"
  end
end

# Create a new post
post "/posts" do |req, params: PostInputSchema|
  post = Post.create!(params)
  req.created json: post.to_h, as: PostResponseSchema
end

# Update an existing post
put "/posts/:id" do |req, params: PostInputSchema|
  post = Post.find(req.url_params[:id])
  if post
    post.update!(params)
    req.ok json: post.to_h, as: PostResponseSchema
  else
    req.not_found "Post not found"
  end
end

# Delete a post
delete "/posts/:id" do |req|
  if Post.delete(req.url_params[:id])
    req.ok json: { message: "Deleted" }
  else
    req.not_found "Post not found"
  end
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

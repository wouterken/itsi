require "json"
include "../helpers/datastore"

# List all users
get "/users" do |req|
  users = User.all.map(&:to_h)
  req.ok json: users
end

# Get a single user
get "/users/:id" do |req|
  user = User.find(req.url_params[:id])
  if user
    req.ok json: user.to_h
  else
    req.not_found "User not found"
  end
end

# Create a new user
post "/users" do |req, params|
  user = User.create!(params.transform_keys(&:to_sym))
  req.created json: user.to_h
end

# Update an existing user
put "/users/:id" do |req|
  user = User.find(req.url_params[:id])
  if user
    user.update!(params)
    req.ok json: user.to_h
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
  req.ok json: posts
end

# Get a single post
get "/posts/:id" do |req|
  post = Post.find(req.url_params[:id])
  if post
    req.ok json: post.to_h
  else
    req.not_found "Post not found"
  end
end

# Create a new post
post "/posts" do |req, params|
  post = Post.create!(params.transform_keys(&:to_sym))
  req.created json: post.to_h
end

# Update an existing post
put "/posts/:id" do |req|
  post = Post.find(req.url_params[:id])
  if post
    post.update!(params)
    req.ok json: post.to_h
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

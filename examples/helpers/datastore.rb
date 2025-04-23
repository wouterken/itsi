# Dummy in-memory data store
# for demo purposes only.
module DataStore
  @users = {}
  @posts = {}
  class << self
    attr_accessor :users, :posts
  end
end

# Models
class User
  attr_accessor :id, :name, :email

  def initialize(id:, name:, email:)
    @id, @name, @email = id, name, email
  end

  def to_h
    { id: id, name: name, email: email }
  end

  def self.all
    DataStore.users.values
  end

  def self.find(id)
    DataStore.users[id.to_i]
  end

  def self.create!(params)
    id = (DataStore.users.keys.max || 0) + 1
    user = new(id: id, name: params[:name], email: params[:email])
    DataStore.users[id] = user
    user
  end

  def update!(params)
    self.name  = params[:name]  if params[:name]
    self.email = params[:email] if params[:email]
    self
  end

  def self.delete(id)
    DataStore.users.delete(id.to_i)
  end
end

class Post
  attr_accessor :id, :title, :body

  def initialize(id:, title:, body:)
    @id, @title, @body = id, title, body
  end

  def to_h
    { id: id, title: title, body: body }
  end

  def self.all
    DataStore.posts.values
  end

  def self.find(id)
    DataStore.posts[id.to_i]
  end

  def self.create!(params)
    id = (DataStore.posts.keys.max || 0) + 1
    post = new(id: id, title: params[:title], body: params[:body])
    DataStore.posts[id] = post
    post
  end

  def update!(params)
    self.title = params[:title] if params[:title]
    self.body  = params[:body]  if params[:body]
    self
  end

  def self.delete(id)
    DataStore.posts.delete(id.to_i)
  end
end

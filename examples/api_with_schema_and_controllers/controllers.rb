class UserController
  # GET /users
  def index(req)
    req.ok json: User.all.map(&:to_h), as: Array[UserResponseSchema]
  end

  # GET /users/:id
  def show(req)
    user = User.find(req.url_params[:id])
    return req.not_found("User not found") unless user

    req.ok json: user.to_h, as: UserResponseSchema
  end

  # POST /users
  def create(req, params: UserInputSchema, response_schema: UserResponseSchema)
    user = User.create!(params)
    req.created json: user.to_h, as: response_schema
  end

  # PUT /users/:id
  def update(req, params: UserInputSchema)
    user = User.find(req.url_params[:id])
    return req.not_found("User not found") unless user

    req.ok json: user.update!(params).to_h, as: UserResponseSchema
  end

  # DELETE /users/:id
  def destroy(req)
    return req.not_found("User not found") unless User.delete(req.url_params[:id])

    req.ok json: { message: "Deleted" }
  end
end

class PostController
  def index(req)
    req.ok json: Post.all.map(&:to_h), as: Array[PostResponseSchema]
  end

  def show(req)
    post = Post.find(req.url_params[:id])
    return req.not_found("Post not found") unless post

    req.ok json: post.to_h, as: PostResponseSchema
  end

  def create(req, params: PostInputSchema)
    post = Post.create!(params)
    req.created json: post.to_h, as: PostResponseSchema
  end

  def update(req, params: PostInputSchema)
    post = Post.find(req.url_params[:id])
    return req.not_found("Post not found") unless post

    req.ok json: post.update!(params).to_h, as: PostResponseSchema
  end

  def destroy(req)
    return req.not_found("Post not found") unless Post.delete(req.url_params[:id])

    req.ok json: { message: "Deleted" }
  end
end

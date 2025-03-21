preload true
workers 1
threads 1

def user_serve(request)
  response = request.response
  response.status = 200
  response.add_header('Content-Type', 'text/plain')
  response << "Hello, user!"
  response.close
end

def user_create(request)
  response = request.response
  response.status = 201
  response.add_header('Content-Type', 'text/plain')
  response << "User created!"
  response.close
end


def organisation_serve(request)
  response = request.response
  response.status = 200
  response.add_header('Content-Type', 'text/plain')
  response << "Hello, user!"
  response.close
end

def organisation_create(request)

end

location "/app" do
  location "/users" do
    get "/:id", :user_serve
    post "/:id", :user_create
  end

  location "/organisations" do
    get "/:id", :organisation_serve

    post "/:id" do |req|
      response = req.response
      response.status = 201
      response.add_header('Content-Type', 'text/plain')
      response << "User created!"
      response.close
    end
  end

  location "/admin*" do
    auth_api_key valid_keys: %w[one two], token_source: { header: { name: 'Authorization', prefix: 'Bearer '}}, error_response: { code: 401, plaintext: 'Unauthorized', default: 'plaintext' }
  end
end

run lambda { |env|
  [200, {}, 'Oh look, it also. Clusters!']
}

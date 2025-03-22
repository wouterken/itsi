workers 1
threads 1

bind 'http://0.0.0.0:3000'

def user_serve(request)
  response = request.response
  response.status = 200
  response.add_header('Content-Type', 'text/plain')
  response << 'Hello, user!'
  response.close
end

def user_create(request)
  response = request.response
  response.status = 201
  response.add_header('Content-Type', 'text/plain')
  response << 'User created!'
  response.close
end

# location '*' do
#   location '/admin*' do
#     auth_api_key valid_keys: %w[one two], token_source: { header: { name: 'Authorization', prefix: 'Bearer ' } },
#                  error_response: { code: 401, plaintext: 'Unauthorized', default: 'plaintext' }
#     run lambda { |env|
#       [200, {}, 'Oh look, it also. Clusters!']
#     }
#   end

#   location '/br' do
#     compress algorithms: ['brotli'], min_size: 0, compress_streams: true, mime_types: ['all'], level: 'fastest'
#     get '/' do |req|
#       req.respond("Hello world. I'm brotli'd!" * 1000)
#     end
#   end

#   location '/zstd' do
#     compress algorithms: ['zstd'], min_size: 0, compress_streams: true, mime_types: ['all'], level: 'fastest'
#     get '/' do |req|
#       req.respond("Hello world. I'm zstd'd!" * 1000)
#     end
#   end

#   location 'gzip' do
#     compress algorithms: ['gzip'], min_size: 0, compress_streams: true, mime_types: ['all'], level: 'fastest'
#     get '/' do |req|
#       req.respond("Hello world. I'm gzip'd!" * 1000)
#     end
#   end

#   location 'deflate' do
#     compress algorithms: ['deflate'], min_size: 0, compress_streams: true, mime_types: ['all'], level: 'fastest'
#     get '/' do |req|
#       req.respond("Hello world. I'm deflated!" * 1000)
#     end
#   end

#   get '/' do |req|
#     req.respond("Hello world. I'm deflated!")
#   end
# end

run lambda { |env|
  [200, {}, 'Oh look, it also. Clusters!']
}

# location '*', protocols: :http do
#   location 'foo' do
#     redirect to: 'https://{host}:3001{path}'
#   end
# end

# location '*', hosts: ['127.0.0.1'], ports: [3001] do
#   proxy to: ['https://docs.rs{path}{query}'],
#         headers: { 'Host' => { value: 'docs.rs' }, 'Origin' => { value: 'https://docs.rs' } }, verify_ssl: false, timeout: 5
# end

# location '*', hosts: ['127.0.0.1'], ports: [3002] do
#   proxy to: ['https://httpbin.org{path}{query}'],
#         headers: { 'Host' => { value: 'docs.rs' }, 'Origin' => { value: 'https://docs.rs' } }, verify_ssl: false, timeout: 5
# end

# location '/app' do
#   location '/users' do
#     get '/:id', :user_serve
#     post '/:id', :user_create
#   end

#   include 'organisations_controller'

#   location '/admin*' do
#     auth_api_key valid_keys: %w[one two], token_source: { header: { name: 'Authorization', prefix: 'Bearer ' } },
#                  error_response: { code: 401, plaintext: 'Unauthorized', default: 'plaintext' }
#   end
# end

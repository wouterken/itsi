workers 1
threads 1

preload false

bind 'http://localhost:3000'
bind 'http://0.0.0.0:8000'

shutdown_timeout 3

auto_reload_config!
log_target :stdout
log_format :plain
log_level :info

require_relative 'echo_service/echo_service_impl'

grpc EchoServiceImpl.new do
  rate_limit requests: 5, seconds: 5, key: 'address',
             store_config: { redis: { connection_url: 'redis://localhost:6379/0' } }, error_response: { plaintext: 'no way', code: 429, default: 'plaintext' }
end

foo_bar(1)

location '/' do
  # compress algorithms: ['zstd'], min_size: 0, compress_streams: true, mime_types: ['all'], level: 'fastest'
  get '/' do |req|
    req.respond('Foo')
  end
  etag type: 'weak', algorithm: 'md5', min_body_size: 0
  post '/' do |req|
    req.respond('Foo')
  end
end

location '/' do
end
# rate_limit requests: 5, seconds: 5, key: 'address', store_config: { redis: { connection_url: 'redis://localhost:6379/0' } }, error_response: { plaintext: 'no way', code: 429, default: 'plaintext' }

#

# static_assets\
#   relative_path: true,
#   root_dir: 'spa/dist',
#   # # Only allow certain file extensions
#   # allowed_extensions: %w[css js jpg jpeg png gif svg ico woff woff2 ttf
#   #                        otf html],
#   # Return a 404 error if file is not found
#   not_found_behavior: { index: 'index.html' },
#   # Enable auto-indexing of directories
#   auto_index: true,
#   # Try adding .html extension to extensionless URLs
#   try_html_extension: true,
#   # Files under this size are cached in memory
#   max_file_size_in_memory: 1024 * 1024, # 1MB
#   # Maximum number of files to keep in memory cache
#   max_files_in_memory: 1000,
#   # Check for file modifications every 5 seconds
#   file_check_interval: 5,
#   # Add custom headers to all responses
#   headers: {
#     'Cache-Control' => 'public, max-age=86400',
#     'X-Content-Type-Options' => 'nosniff'
#   }

run(lambda  { |env|
  [200, {}, ["I'm a fallback"]]
})

# def user_serve(request)
#   response = request.response
#   response.status = 200
#   response.add_header('Content-Type', 'text/plain')
#   response << 'Hello, user!'
#   response.close
# end

# def user_create(request)
#   response = request.response
#   response.status = 201
#   response.add_header('Content-Type', 'text/plain')
#   response << 'User created!'
#   response.close
# end

# location "/etag-test" do
#   etag type: 'strong', algorithm: 'sha256', min_body_size: 0

#   get '/' do |req|
#     # Fixed content that will generate the same ETag each time
#     content = "This is a fixed response that will always have the same ETag."

#     # Add a timestamp as a comment to see the response is fresh
#     resp = req.response
#     req.respond("Content: #{content}.# Generated at: #{Time.now}")
#   end
# end

# location "/cached-with-etag" do
#   # Set up caching parameters
#   cache_control max_age: 3600, public: true, vary: ['Accept-Encoding']

#   # Add ETags for cache validation
#   etag type: 'weak', algorithm: 'md5', min_body_size: 0

#   get '/' do |req|
#     req.respond("This response will have both cache headers and an ETag.")
#   end
# end

# location "/" do
#   intrusion_protection banned_time_seconds: 5, banned_url_patterns: [/\/admin/, /\/secret/], store_config: { redis: { connection_url: 'redis://localhost:6379/0' } }, error_response: { plaintext: 'no way', code: 403, default: 'plaintext' }

#   # Example for API endpoints with ETags
#   location "/api" do
#     etag type: 'weak', algorithm: 'md5', min_body_size: 0
#     get '/users/id' do |req|
#       user_id = 5
#       req.respond("User data for #{user_id}")
#     end
#   end

#   location "/hey" do
#     location "/world*" do
#       request_headers additions: {"X-Custom-Req" => ["Foo", "Bar", "Baz"]}, removals: [""]
#       response_headers additions: {"X-Custom-Resp" => ["Foo", "Bar"]}, removals: []
#       rate_limit requests: 5, seconds: 30, key: 'address', store_config: { redis: { connection_url: 'redis://localhost:6379/0' } }, error_response: { plaintext: 'no way', code: 429, default: 'plaintext' }
#       post '/' do |req|
#         puts "I'm a post!"
#         req.respond("Post successful!")
#       end
#       get '/' do |req|
#         req.respond("Is this still fast?")
#       end
#     end
#   end

#   # Add a new location with cache_control middleware
#   location "/cached" do
#     cache_control max_age: 3600, public: true, vary: ['Accept-Encoding']
#     get '/' do |req|
#       req.respond("This content is cached for 1 hour.")
#     end
#   end

#   location "/etag" do
#     etag enabled: true, etag_type: 'strong', hash_algorithm: 'sha256', min_body_size: 0
#     get '/' do |req|
#       # Create a response large enough to be worth ETagging
#       content = "This content will have ETags. " * 20
#       req.respond(content)
#     end
#   end

#   location "/jwt" do
#     auth_jwt token_source: { header: { name: 'Authorization', prefix: 'Bearer ' } },
#       verifiers: {
#         'hs256' => ["1V6nZzztO/F6Y3XAYXJ1I37AAXCJ/V7EVHJoVnD8lr4="],
#         'rs256' => [%Q{
#           -----BEGIN PUBLIC KEY-----
#           MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAs8XK+wP7nnKkOGXP6+il
#           +JDQMFRV8zoVg1BYYapGla6AzR0YCV6xq28lNB1FE8C6xhCEKYiOlnkXZwj+a4UW
#           q4Bex/U9qvfza0zGxHlTnQT9T+vjwXNsWYuefYC4EAZjEd6bGeP23De128QfhlLc
#           oQpcfW32/e8KjSYtn8rtWuKV6Anqf92o9zEbVn8y2OZwZbiW0LbfDnimazqP5ATy
#           SI6a5kQorBeG9cU4WW93ctuT5nkyAbBaEpiejWIpjbMkofD5fj2pUgg7H/Xvh+eR
#           S9A+//VGfy4A4SFjBooVewhJ04VhDkMnVC29fbVwVuNqm6Z+FZaxI5pqvisxvH8S
#           XwIDAQAB
#           -----END PUBLIC KEY-----
#           }]
#   },  error_response: { code: 401, plaintext: 'Unauthorized', default: 'plaintext' }
#     get "/" do |req|
#       req.respond('Hello, JWT user!')
#     end
#   end

# end

# # For an assets example
# location "/assets" do
#   cache_control max_age: 604800, s_max_age: 2592000,
#                stale_while_revalidate: 86400,
#                public: true, immutable: true

#   get "/:file" do |req|
#     file_name = req.params["file"]
#     req.respond("Pretending to serve #{file_name} with long-lived caching")
#   end
# end

# # location '' do

#   # location '/spa' do
#   #   # Serve static files from the "public/assets" directory
#   #   log_requests before: { format: "REQUEST_ID={request_id}  METHOD={method} PATH={path} QUERY={query} HOST={host} PORT={port} START_TIME={start_time}" , level: 'INFO'}, after: { format: "REQUEST_ID={request_id} RESPONSE_TIME={response_time}", level: 'INFO' }
#   #   allow_list allowed_patterns: [/127.*/], error_response: { plaintext: 'no way', code: 401, default: 'plaintext' }
#   #   static_assets\
#   #     relative_path: true,
#   #     root_dir: 'spa/dist',
#   #     # # Only allow certain file extensions
#   #     # allowed_extensions: %w[css js jpg jpeg png gif svg ico woff woff2 ttf
#   #     #                        otf html],
#   #     # Return a 404 error if file is not found
#   #     not_found_behavior: { index: 'index.html' },
#   #     # Enable auto-indexing of directories
#   #     auto_index: true,
#   #     # Try adding .html extension to extensionless URLs
#   #     try_html_extension: true,
#   #     # Files under this size are cached in memory
#   #     max_file_size_in_memory: 1024 * 1024, # 1MB
#   #     # Maximum number of files to keep in memory cache
#   #     max_files_in_memory: 1000,
#   #     # Check for file modifications every 5 seconds
#   #     file_check_interval: 5,
#   #     # Add custom headers to all responses
#   #     headers: {
#   #       'Cache-Control' => 'public, max-age=86400',
#   #       'X-Content-Type-Options' => 'nosniff'
#   #     }
#   #   compress algorithms: ['zstd'], min_size: 0, compress_streams: true, mime_types: ['all'], level: 'fastest'
#   # end
# # end

# # location "/cors_test" do
# #   cors allowed_origins: ["http://127.0.0.1:8000"], allowed_methods: ["GET", "PATCH", "POST"], allowed_headers: ['Content-Type'], exposed_headers: ['X-Custom-Header'], allow_credentials: true
# #   get "/user/:id" do |req|
# #     req.respond("You've been CORSed!")
# #   end
# # end

# # location "/basic" do
# #   auth_basic realm: 'My Realm', credential_pairs: { 'user' => 'password' }
# # end

# # location '/proxy_as_foo_com' do
# #   compress algorithms: ['zstd'], min_size: 0, compress_streams: true, mime_types: ['all'], level: 'fastest'
# #   proxy to: 'http://foo.com/zstd', backends: ['http://127.0.0.1:3000'], headers: { 'Host' => { 'value' => 'foo.com' } },
# #         verify_ssl: false, timeout: 100, tls_sni: true
# # end

# # location '/proxy_as_bar_com' do
# #   proxy to: 'http://bar.com', backends: ['http://127.0.0.1:3000'], headers: { 'Host' => { 'value' => 'bar.com' } },
# #         verify_ssl: false, timeout: 100, tls_sni: true
# # end

# # location '/', hosts: ['foo.com'] do
# #   get '/' do |req|
# #     req.respond('I am foo.com')
# #   end
# # end

# # location '/', hosts: ['bar.com'] do
# #   get '/' do |req|
# #     req.respond('I am bar.com')
# #   end
# # end

# # location '/admin*' do
# #   auth_api_key valid_keys: %w[one two], token_source: { header: { name: 'Authorization', prefix: 'Bearer ' } },
# #                error_response: { code: 401, plaintext: 'Unauthorized', default: 'plaintext' }
# #   run lambda { |env|
# #     [200, {}, 'Oh look, it also. Clusters!']
# #   }
# # end

# # location '/br' do
# #   compress algorithms: ['brotli'], min_size: 0, compress_streams: true, mime_types: ['all'], level: 'fastest'
# #   get '/' do |req|
# #     req.respond("Hello world. I'm brotli'd!")
# #   end
# # end

# # location '/zstd' do
# #   compress algorithms: ['zstd'], min_size: 0, compress_streams: true, mime_types: ['all'], level: 'fastest'
# #   get '/' do |req|
# #     req.respond("Hello world. I'm zstd'd!")
# #   end
# # end

# # location 'gzip' do
# #   compress algorithms: ['gzip'], min_size: 0, compress_streams: true, mime_types: ['all'], level: 'fastest'
# #   get '/' do |req|
# #     req.respond("Hello world. I'm gzip'd!")
# #   end
# # end

# # location 'deflate' do
# #   compress algorithms: ['deflate'], min_size: 0, compress_streams: true, mime_types: ['all'], level: 'fastest'
# #   get '/' do |req|
# #     req.respond("Hello world. I'm deflated!")
# #   end
# # end

# # get '/hey' do |req|
# #   req.respond("This is a test")
# # end

# # location '*', protocols: :http do
# #   location 'foo' do
# #     redirect to: 'https://{host}:3001{path}'
# #   end
# # end

# # location '*', hosts: ['127.0.0.1'], ports: [3001] do
# #   proxy to: ['https://docs.rs{path}{query}'],
# #         headers: { 'Host' => { value: 'docs.rs' }, 'Origin' => { value: 'https://docs.rs' } }, verify_ssl: false, timeout: 5
# # end

# # location '*', hosts: ['127.0.0.1'], ports: [3002] do
# #   proxy to: ['https://httpbin.org{path}{query}'],
# #         headers: { 'Host' => { value: 'docs.rs' }, 'Origin' => { value: 'https://docs.rs' } }, verify_ssl: false, timeout: 5
# # end

# # location '/app' do
# #   location '/users' do
# #     get '/:id', :user_serve
# #     post '/:id', :user_create
# #   end

# #   include 'organisations_controller'

# #   location '/admin*' do
# #     auth_api_key valid_keys: %w[one two], token_source: { header: { name: 'Authorization', prefix: 'Bearer ' } },
# #                  error_response: { code: 401, plaintext: 'Unauthorized', default: 'plaintext' }
# #   end
# # end
# # foo
# # foo

# # Example of using ETag middleware
# # location "/with-etag" do
# #   etag
# #   get '/' do |req|
# #     req.respond("This response will have an ETag header based on content hash.")
# #   end
# # end

# # Example with both caching and ETags

# # Simple ETag test endpoint with predictable content

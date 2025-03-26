# Example of how to use the Static Assets middleware in Itsi.rb

workers 1
threads 1
bind 'http://localhost:3000'

# Example 1: Using the "alias" behavior (default)
# With alias behavior, if you access /assets/image.png, it looks for the file at public/assets/image.png
location '/assets' do
  # Serve static files from the "public/assets" directory
  static_assets root_dir: "public/assets",
                # Default behavior is "alias" - the location pattern is stripped from the path
                # root_is_prefix: false,  # This is the default, so we don't need to specify it
                # Only allow certain file extensions
                allowed_extensions: %w[css js jpg jpeg png gif svg ico woff woff2 ttf otf html],
                # Return a 404 error if file is not found
                not_found: 'error',
                # Enable auto-indexing of directories
                auto_index: true,
                # Try adding .html extension to extensionless URLs
                try_html_extension: true,
                # Files under this size are cached in memory
                max_file_size_in_memory: 1024 * 1024, # 1MB
                # Maximum number of files to keep in memory cache
                max_files_in_memory: 1000,
                # Check for file modifications every 5 seconds
                file_check_interval: 5,
                # Add custom headers to all responses
                headers: {
                  'Cache-Control' => 'public, max-age=86400',
                  'X-Content-Type-Options' => 'nosniff'
                }
end

# Example 2: Using the "root" behavior
# With root behavior, if you access /static/image.png, it looks for the file at public/static/image.png
location '/static' do
  # Serve static files from the "public" directory
  static_assets root_dir: "public",
                # Use "root" behavior - keep the location pattern as part of the path
                root_is_prefix: true,
                not_found: 'error',
                auto_index: true
end

# Example 3: Serving a Single Page Application (SPA)
location '/app' do
  static_assets root_dir: "public/app",
                # If file is not found, serve index.html (typical SPA behavior)
                not_found: "index", # equivalent to not_found: { index: "index.html" }
                headers: {
                  'Cache-Control' => 'public, max-age=3600'
                }
end

# Example 4: Using file_server as an alias for static_assets
location '/public' do
  file_server root_dir: "public",
              auto_index: true,
              not_found: "fallthrough"
end

# Example of a restricted files area
location '/downloads' do
  # Basic authentication before serving files
  auth_basic realm: 'Downloads Area', credential_pairs: { 'user' => 'password' }

  # Serve files after authentication
  static_assets root_dir: 'private/downloads',
                # Only allow PDF files
                allowed_extensions: ['pdf'],
                # If file is not found, fall through to the next middleware
                not_found: 'fallthrough'
end

# Example of redirecting to another location if file is not found
location '/docs' do
  static_assets root_dir: 'documentation',
                # Redirect to documentation homepage if file not found
                not_found: 'redirect', # equivalent to not_found: { redirect: "/docs/index.html" }
                # Don't cache these files in memory
                max_file_size_in_memory: 0
end

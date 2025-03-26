# Static Assets Middleware

The Static Assets middleware allows you to serve static files directly from the filesystem with high performance. It includes support for directory listings, range requests for media files, automatic MIME type detection, and configurable caching.

## Features

- **Efficient file serving**: Small files are cached in memory, large files are streamed directly from disk
- **Range requests**: Support for HTTP Range headers, essential for media streaming
- **MIME type detection**: Automatic content type detection based on file extensions
- **Directory traversal protection**: Guards against path traversal attacks
- **Configurable auto-indexing**: Optional directory listing capabilities similar to nginx
- **Extensionless URLs**: Optional support for trying `.html` extension when a file is not found
- **Custom error handling**: Configure behavior for not-found files (error, fallthrough, redirect, or serve index)
- **Fine-grained control**: Restrict by file type, set custom headers, configure caching behavior
- **Single Page Application (SPA) support**: Automatically serve an index file for missing URLs

## Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `root_dir` | String | `.` | Root directory from which to serve files |
| `allowed_extensions` | Array<String> | nil | List of file extensions to serve (if nil, all extensions are allowed) |
| `not_found` | String/Hash | `"error"` | What to do when a file is not found. Options: `"error"`, `"fallthrough"`, `"index"`, `"redirect"` |
| `auto_index` | Boolean | false | Whether to show directory listings when a directory is requested |
| `try_html_extension` | Boolean | true | Try adding .html extension to extensionless URLs |
| `max_file_size_in_memory` | Integer | 1048576 (1MB) | Files below this size will be cached in memory |
| `max_files_in_memory` | Integer | 1000 | Maximum number of files to keep in memory cache |
| `file_check_interval` | Integer | 5 | How often to check for file changes (in seconds) |
| `headers` | Hash | nil | Additional headers to include with all responses |

### Not Found Behavior Options

The `not_found` option can be one of:

- `"error"` - Return a 404 error
- `"fallthrough"` - Pass the request to the next middleware in the stack
- `"index"` - Serve a designated index file (e.g., `index.html`)
- `"redirect"` - Redirect to a specified URL

When using `"index"` or `"redirect"`, you can provide a string directly (e.g., `not_found: "index"` defaults to `index.html`) or a hash with more specific configuration (e.g., `not_found: { index: "custom-index.html" }`).

## Usage Examples

### Basic Static File Server

```ruby
location "/assets" do
  static_assets root_dir: "public/assets",
               allowed_extensions: ["css", "js", "jpg", "jpeg", "png", "gif", "svg", "ico"],
               not_found: "error",
               auto_index: false
end
```

### Single Page Application (SPA)

```ruby
location "/" do
  static_assets root_dir: "public",
               # If file not found, serve index.html (SPA fallback)
               not_found: { index: "index.html" },
               # Cache aggressively for better performance
               max_file_size_in_memory: 5 * 1024 * 1024, # 5MB
               max_files_in_memory: 1000,
               # Add caching headers
               headers: {
                 "Cache-Control" => "public, max-age=3600"
               }
end
```

### Directory Listing

```ruby
location "/downloads" do
  static_assets root_dir: "downloads",
               auto_index: true,
               # Don't cache these files in memory
               max_file_size_in_memory: 0
end
```

### Restricted File Types

```ruby
location "/documents" do
  # Only serve PDF and text files
  static_assets root_dir: "documents",
               allowed_extensions: ["pdf", "txt"],
               not_found: "error"
end
```

### File Server with Custom Headers

```ruby
location "/media" do
  static_assets root_dir: "media",
               headers: {
                 "Cache-Control" => "public, max-age=604800", # 1 week
                 "X-Content-Type-Options" => "nosniff"
               }
end
```

## Performance Considerations

- Files smaller than `max_file_size_in_memory` are cached entirely in memory, making subsequent accesses extremely fast
- The cache size is limited to `max_files_in_memory` entries using an LRU (Least Recently Used) algorithm
- Files larger than `max_file_size_in_memory` are streamed directly from disk
- Range requests are handled efficiently, reading only the requested portion of the file
- The file modification time is checked every `file_check_interval` seconds to detect changes

## Security Considerations

- The middleware has built-in protection against directory traversal attacks
- When `allowed_extensions` is configured, only files with the specified extensions are served
- File access is limited to the specified `root_dir` 
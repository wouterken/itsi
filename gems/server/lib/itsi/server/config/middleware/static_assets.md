---
title: Static Assets
url: /middleware/static_assets
---

The Static Assets middleware serves files from a specified root directory. It is capable of optimized delivery of static content such as HTML, CSS, JavaScript, images, as well as large assets, such as video files using streaming bodies and range requests.
It can auto-index directories for simple directory listings.

## Key Features
- **Auto Indexing**: Optionally generate directory listings when an index file is missing.
- **HTML Fallback**: When enabled, the middleware attempts to serve a file with a `.html` extension if the requested file is not found.
- **In-Memory Caching**: Files under a certain size (and up to a configurable count) are cached in memory for performance.
- **Custom Headers**: User-supplied headers (e.g., for caching) can be added to responses.
- **Relative Path Processing**: When enabled, the middleware rewrites request paths relative to a configured base path.
- **Partial Content**: Supports Range requests for serving partial file content.

## Example

```ruby {filename=Itsi.rb}
static_assets root_dir: "./"
```

### Directory Index
#### HTML

  {{< card link="/" title="Static File Server" image="/directory_listing.jpg" subtitle="Static File Listing, Powered by Itsi." method="Resize" options="500x q80 webp" >}}

#### JSON
Directory indexes also support responding in JSON format. E.g.

`curl -H "Accept: application/json" http://0.0.0.0`

```json
{
  "directory": ".",
  "items": [
    {
      "is_dir": false,
      "modified": "2025-04-22 04:21:43",
      "name": "Gemfile",
      "path": "Gemfile",
      "size": "42 B"
    },
    {
      "is_dir": false,
      "modified": "2025-04-22 04:21:48",
      "name": "Gemfile.lock",
      "path": "Gemfile%2Elock",
      "size": "463 B"
    },
    {
      "is_dir": false,
      "modified": "2025-04-22 02:46:45",
      "name": "Itsi.rb",
      "path": "Itsi%2Erb",
      "size": "80 B"
    }
  ],
  "title": "Directory listing for ."
}
```

## Configuration Options


| Option                         | Description                                                                                                                                                                                                                           |
|--------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **`root_dir`** (Default ".")                | The relative or absolute disk path where static files reside.                                                                                                                                                                                   |
| **`not_found_behavior`** (Default `{error: "not_found"}`)       | Determines what to do when a file isn’t found. See [Not Found Behavior](#not-found-behavior) below. |
| **`auto_index`** (Default `false`)            | If <code>true</code>, directory listings are automatically generated when no index file is found. Defaults to <code>false</code>.                                                                                                    |
| **`try_html_extension`** (Default `true`)      | If <code>true</code>, when the requested file isn’t found, the middleware attempts to serve the same path with a <code>.html</code> extension. Defaults to <code>true</code>.                                                 |
| **`max_file_size_in_memory`** (Default `1048576`)  | Maximum file size (in bytes) for caching files in memory. Files larger than this will be served from disk. Defaults to <code>1048576</code> (1 MB).                                                                                |
| **`max_files_in_memory`** (Default `100`)      | Maximum number of files to cache in memory. Defaults to <code>100</code>.                                                                                                                                                             |
| **`file_check_interval`** (Default `1`)      | Max time (in seconds) a file stays in cache before the file-system is checked for modifications. Defaults to <code>1</code>.                                                                                                                                                |
| **`headers`** (Default `nil`)                 | A hash of additional headers to include in responses (e.g. <code>{"Cache-Control" => "max-age=3600"}</code>).                                                                                                                         |
| **`allowed_extensions`** (Default `[]`)       | An array of permitted file extensions (e.g. <code>["html", "css", "js", "png", "jpg"]</code>). If a requested file’s extension isn’t in this list, it won’t be served. An empty list (default) means all extensions are allowed.                                                           |
| **`relative_path`** (Default `true`)           | If <code>true</code>, the effective file lookup path is computed relative to the base path at which the middleware is mounted (See [location](/middleware/location) to understand mounting options). Defaults to <code>true</code>.                                                                                               |
| **`serve_hidden_files`** (Default `false`)       | If <code>false</code>, files whose names start with a dot (".") are not served. Defaults to <code>false</code>.                                                                                                                        |


## Not Found Behavior
The static assets middleware supports many configuration options when a file is not found.
* **`fallthrough`**: The request is *not* handled by this middleware and is instead served by the next middleware in the chain. The option is given as a string. E.g.
```ruby
error_response: "fallthrough"
```
* **`index`**: Unsatisfiable requests instead result in serving an index file. (Useful for SPAs with client-side routing).
```ruby
error_response: {index: "./path/to/index.html" }
```
* **`redirect`**: Unsatisfiable requests are redirected. Options for type are the same as defined for the [redirect](/middleware/redirect) middleware.
```ruby
error_response: {redirect: { to: "http://example.com/redirect_path", type: "permanent" }}
```
* **`error`**: Provide an overridden error response. See [`error_response`](/middleware/error_response) for details.
e.g.
```ruby
error_response: {error: "not_found"}
```

```ruby
error_response: \
  {
    error:
    {
      code: 404,
      plaintext: {inline: "File not found"},
      json: {inline: "{\"message\": \"File not found\"}"},
      html: {file: "./path/to/not_found.html"},
      default: "plaintext"
    }
  }
```

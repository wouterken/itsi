---
title: HTTP Request
url: /middleware/http_request
---

An [endpoint](/middleware/endpoint), always accepts a  `request` object as the first parameter.
E.g.

```ruby {filename=Itsi.rb}
get "/" do |req|
end
```


end

| Method           | Description                                                                 |
|------------------|-----------------------------------------------------------------------------|
| `path`           | Retrieves the path of the HTTP request.                                    |
| `script_name`    | Retrieves the script name of the HTTP request.                             |
| `query_string`   | Retrieves the query string from the HTTP request.                          |
| `content_type`   | Retrieves the content type of the HTTP request.                            |
| `content_length` | Retrieves the content length of the HTTP request.                          |
| `request_method` | Retrieves the HTTP method (e.g., GET, POST) of the request.                |
| `version`        | Retrieves the HTTP version of the request.                                 |
| `rack_protocol`  | Retrieves the Rack protocol version used in the request.                   |
| `host`           | Retrieves the host of the HTTP request.                                    |
| `headers`        | Retrieves all headers from the HTTP request.                               |
| `uri`            | Retrieves the full URI of the HTTP request.                                |
| `header`         | Retrieves the value of a specific header from the HTTP request.            |
| `[]`             | Alias for `header`, retrieves the value of a specific header.              |
| `scheme`         | Retrieves the scheme (e.g., http, https) of the HTTP request.              |
| `remote_addr`    | Retrieves the remote address of the client making the request.             |
| `port`           | Retrieves the port number of the HTTP request.                             |
| `body`           | Retrieves the body of the HTTP request (As an IO).                         |
| `response`       | Retrieves the [response](/middleware/http_response) object associated with the HTTP request.            |
| `json?`          | Checks if the request content type is JSON.                                |
| `html?`          | Checks if the request content type is HTML.                                |
| `url_encoded?`   | Checks if the request content type is URL-encoded.                         |
| `multipart?`     | Checks if the request content type is multipart.                           |
| `url_params`     | Retrieves the URL parameters from the HTTP request.                        |
| `#<status_name>` | Writes a response with the specified status code and closes the response. |
| `respond`       | Writes a response with the specified status code and closes the response. |
| `query_params`     | Retrieves the query parameters from the HTTP request.                      |

---
title: Endpoint
url: /middleware/endpoint
prev: deny_list/
next: controller/
---

The **endpoint** middleware allows you to define an ultra light-weight, inline, Ruby endpoint.

> If you're after running a rack app using a fully-featured framework, e.g. a Ruby on Rails or Sinatra, take a look at the [Rackup File](/middleware/rackup_file) middleware instead.
This feature can be useful for quickly prototyping, building small pieces of isolated functionality, or minimal endpoints where high throughput is essential.

`endpoint` has several variants, that further restrict the endpoint to respond to specific HTTP methods:
- [`get`](/middleware/get) for **GET** requests
- [`post`](/middleware/post) for **POST** requests
- [`put`](/middleware/put) for **PUT** requests
- [`patch`](/middleware/patch) for **PATCH** requests
- [`delete`](/middleware/delete) for **DELETE** requests

### Functions
Endpoints also support:
* Request and response schema validation. See [Schema Validation](/middleware/endpoint/schemas)
* Controllers. See [Controllers](/middleware/controller)

## Usage
Endpoints require an optional path (default "*") and a handler proc or function, which must accept a mandatory request object (See [Request](/middleware/http_request)) and an optional params object.


```ruby {filename=Itsi.rb}
# A routeless endpoint is the same as a 'catch-all' endpoint.
# E.g. this:
get do |req|
end

# Is equivalent to this:
get "*" do |req|
end
```

The request object itself holds a reference [`#response`](/middleware/http_response) object, which can be used to manage the response explicitly.

### Request Life-cycle
Unlike most Rack frameworks where the life-span of an HTTP request/response is tied to the controller action, in Itsi there is no such contract.
You must explicitly close the response to complete it.
This also allows you hold on to a connection *indefinitely* (or until top-level timeouts occur, e.g. [request_timeout](/options/request_timeout)), and makes it easy to manage several concurrent requests asynchronously (especially if combined with [fiber_scheduler](/options/fiber_scheduler)).

There are several ways to write and close a response.

**Simple Responses**
* `request#respond`.
```ruby
get do |req|
  req.respond "ok", 200, {} # All params are optional, and can also use named kwargs instead of positional args
end
```
* respond + status aliases. E.g. `request#ok`, `request#created`, `request#not_found`
```ruby
get do |req|
  req.ok "ok", {} # All params are optional, and can also use named kwargs instead of positional args
end
```

**Low-level responses** (for low-level control over long-lived requests)
* `response#respond`
* `response#send_and_close`
* `response#close`

#### Simple Responses
For most use-cases using simple responses is all you need.
E.g.

```ruby {filename=Itsi.rb}
# Catch-all endpoint.
endpoint "/example/*" do |request|
  request.ok "Hello, World!"
end
```


```ruby{filename=Itsi.rb}
# Single body, status and headers

# 200 assumed
endpoint("/"){|req| req.respond "Just a body"  }

# With status
endpoint("/"){|req| req.respond "Body and status", 200  }

# With status and headers
endpoint("/"){|req| req.respond "Body and status", 200, {"Content-Type" => "text/plain"}  }

# With kwargs
endpoint("/"){|req| req.respond body: "Just a body"  }

# With status
endpoint("/"){|req| req.respond body: "Body and status", status: 200  }

# With status and headers
endpoint("/"){|req| req.respond body: "Body and status", status: 200, headers: {"Content-Type" => "text/plain"}  }

# Response Formats
# JSON
endpoint("/"){|req| req.respond json: { "message": "With JSON Body" }  }

# XML
endpoint("/"){|req| req.respond xml: "<message>With XML Body</message>"}

# HTML
endpoint("/"){|req| req.respond html: "<html><body><h1>With HTML Body</h1></body></html>"}

# Text
endpoint("/"){|req| req.respond text: "With Text Body"}


# Status helpers (All status codes supported)
endpoint("/"){|req| req.ok "Ok"  }
endpoint("/"){|req| req.not_found "Not Found"  }
endpoint("/"){|req| req.created "Created"  }
endpoint("/"){|req| req.accepted "Accepted"  }
```

For more advanced responses (e.g streaming responses), see documentation on [response](/middleware/response.rb)

### Capturing URL parameters
```ruby {filename=Itsi.rb}
# Catch-all endpoint.
location "/foo" do
  endpoint "/users/:user_id" do |request|
    if (user = User.find(request.query_params[:user_id]))
      request.ok json: user.to_json
    else
      request.not_found "User not found!"
    end
  end

  # Optionally restrict the character sets of capture groups using Regex
  endpoint "/books/:book_id(\d+)" do |request|
    request.ok "Got book #{request.query_params[:book_id]}"
  end
end
```

### Basic Request Body / Parameters

If an endpoint accepts a second parameters argument, incoming request bodies will be parsed into a Ruby hash (including uploaded files as `File` objects and fed into the handler as the second parameter ).

The following request formats will be automatically detected and deserialized:
* **JSON** (`"Content-Type"` header is set to `"application/json"`)
* **Form-encoded** (`"Content-Type"` header is set to `"application/x-www-form-urlencoded"`)
* **Multipart** (`"Content-Type"` header is set to `"multipart/form-data"`)


```ruby {filename=Itsi.rb}
location "/echo" do
  post "/body" do |request, params|
    request.ok json: params.to_json
  end
end
```

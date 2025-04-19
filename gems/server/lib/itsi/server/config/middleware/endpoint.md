---
title: Endpoint
url: /middleware/endpoint
---

The **endpoint** middleware allows you to define an ultra light-weight inline Ruby endpoint, without having to run a fully-fledged Rack application.
> If you're after running a rack app, e.g. a Ruby on Rails or Sinatra app, take a look at the [Rackup File](/middleware/rackup_file) middleware instead.
This can be useful for quickly prototyping, testing small pieces of functionality, or minimal endpoints where high throughput is essential.

`endpoint` has several variants, that further restrict the endpoint to respond to specific HTTP methods:
- [`get`](/middleware/get) for **GET** requests
- [`post`](/middleware/post) for **POST** requests
- [`put`](/middleware/put) for **PUT** requests
- [`patch`](/middleware/patch) for **PATCH** requests
- [`delete`](/middleware/delete) for **DELETE** requests

### Functions
Endpoints also support:
* Request and response schema validation. See [Schema Validation](#schema-validation)
* Controllers. See [Controllers](#controllers)

## Usage
Endpoint functions must accept a mandatory request object (See [Request](/middleware/http_request))
and an optional params object.

The request object itself holds a `#response` object, which can be used to manage the response explicitly.

It also defines several short-hands for returning simple responses without needing to handle the response explicitly.
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
* **JSON** ("Content-Type" header is set to "application/json")
* **Form-encoded** ("Content-Type" header is set to "application/x-www-form-urlencoded")
* **Multipart** ("Content-Type" header is set to "multipart/form-data")


```ruby {filename=Itsi.rb}
location "/echo" do
  post "/body" do |request, params|
    request.ok json: params.to_json
  end
end
```

### Schema Validation
Endpoints also support basic schema enforcement for both requests and responses.

Endpoint Schemas are simply plain-old Ruby hashes, which map named keys to allowed types, with a few special rules/behaviours:

* Regexp patterns are used in place of symbol keys to define dynamic parameters that must match a specific pattern (A /.*/ matches any string).
* A `:Boolean` symbol can be used instead of a class to require boolean parameters (as a proxy for the combination of TrueClass and FalseClass)
* `Array` keys are used to define arrays of values.
* Objects can be nested
* A special `_required` key indicates which parameters in the current object are required.
* Arrays must be homogeneous (No support for union types).

```ruby {filename=Itsi.rb}
AddressSchema = {
  _required: %i[street city postcode],
  street: String,
  city: String,
  postcode: String,
  country: String # Optional
}

UserInputSchema = {
  _required: %i[first_name last_name email address],
  first_name: String,
  last_name: String,
  email: String,
  age: Integer,
  active: :Boolean,
  roles: Array[String],
  address: AddressSchema
}

UserResponseSchema = {
  _required: %i[id email full_name],
  id: Integer,
  email: String,
  full_name: String,
  created_at: String,
  address: AddressSchema
}

post "/users" do |request, params: UserInputSchema, response_format: UserResponseSchema|
  user = User.create!(params)
  request.created \
    json: {
      id: user.id,
      email: user.email,
      full_name: "#{user.first_name} #{user.last_name}",
      created_at: user.created_at.iso8601,
      address: {
        street: user.address.street,
        city: user.address.city,
        postcode: user.address.postcode
      }
    },
    as: response_format
end
```
Endpoints with schema validation applied can count on requests *only* being invoked with correctly formed parameters (and descriptive errors being returned if schema enforcement fails.)

You can *optionally* choose to apply a corresponding response schema validation to response objects.

### Controllers
Instead of supplying the endpoint body inline, you can also reference a method by name (as a symbol).
Itsi will attach the method, resolved by name within the current controller scope, to the endpoint.

The default controller scope is simply the parent `Itsi.rb` config file.

However you can use the `controller` middleware to explicitly set a controller scope per [location](/middleware/location) block.

{{< callout  >}}
There are no special requirements for a controller in Itsi, it can be *any* ruby object, so long as it responds to the methods attached to the endpoint and accepts the request object as the first argument. This could be a singleton module, an instance, a struct etc.
{{< /callout >}}.

E.g.

```ruby {filename=Itsi.rb}
class UsersController

  def initialize()
    # One time controller set-up here
  end

  def create(request, params: UserInputSchema, response_format: UserResponseSchema)
    user = User.create!(params)
    request.created \
      json: {
        id: user.id,
        email: user.email,
        full_name: "#{user.first_name} #{user.last_name}",
        created_at: user.created_at.iso8601,
        address: {
          street: user.address.street,
          city: user.address.city,
          postcode: user.address.postcode
        }
      },
      as: response_format
  end
end

controller UsersController.new
post "/users", :create
```

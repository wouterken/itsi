---
title: Controller
url: /middleware/controller
---


Instead of supplying an [endpoint](/middleware/endpoint) body inline, you can also reference a method by name (as a `:symbol`).
Itsi will attach the method, resolved by name within the current controller scope, to the endpoint.

E.g.
```ruby {filename=Itsi.rb}
controller UserController.new
get "", :index
```

The default controller scope is simply the parent `Itsi.rb` config file.
However you can use the `controller` middleware to explicitly set a controller scope per [location](/middleware/location) block. All named endpoints will then be satisfied by the current controller.

{{< callout >}}
Itsi will check for the presence and structure of all:
* Controller methods
* [Schemas](/middleware/schemas)

at **boot time** ensuring that you can't be caught out by naming mismatches at runtime, or during a hot-reload.
If Itsi boots successfully, the controller methods exist and accept the correct parameters.
{{</ callout >}}

## Basic Example
```ruby {filename=Itsi.rb}
controller UserController.new
```

{{< callout  >}}
There are no special requirements for a controller in Itsi, it can be *any* ruby object, so long as it responds to the methods attached to the endpoint and accepts the request object as the first argument. This could be a singleton module, an instance, a struct etc.
{{< /callout >}}.

### Detailed Example

```ruby {filename=Itsi.rb}

require_relative "schemas"
require_relative "user_controller"

location "/users*" do
  controller UserController.new
  post "/", :create
end

def home(req)
  req.respond "I'm home"
end
# Default controller is just the top-level Itsi scope
get "/", :home
```


```ruby {filename=user_controller.rb}
class UserController

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

# Dummy data model.
Address = Struct.new(:street, :city, :postcode, :country, keyword_init: true)

class User < Struct.new(
  :id, :email, :first_name, :last_name, :created_at,
  :address, :age, :active, :roles, keyword_init: true)
  def self.create!(params)
    self.new(
      id: Random.rand(1000000...99999999),
      address: Address.new(params.delete :address),
      **params,
      created_at: Time.now
    )
  end
end

```

```ruby {filename=schemas.rb}
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
```

```bash
curl http://0.0.0.0:3000/users -H 'Accept: application/json' -H 'Content-Type: application/json' -d '{"id": 3, "age": "nine", "first_name":8,"last_name":"test","email":"test", "address": {"street":"1 Main Street","city":"Wellington","postcode":1234}}'

{"error":"Validation failed: Invalid value for Integer at age: \"nine\" (invalid value for Integer(): \"nine\")"}%
```


```bash
curl http://0.0.0.0:3000/users \
  -H 'Accept: application/json' \
  -H 'Content-Type: application/json' \
  -d '{"id": 3, "age": "ninety-nine", "first_name":"John","last_name":"Smith","email":"test", "address": {"street":"1 Main Street","city":"Wellington","postcode":1234}}'
```
```json
{
  "error":"Validation failed: Invalid value for Integer at age: \"ninety-nine\" (invalid value for Integer(): \"nine\")"}%
```

```bash
curl http://0.0.0.0:3000/users \
  -H 'Accept: application/json' \
  -H 'Content-Type: application/json' \
  -d '{"id": 3, "age": 99, "first_name":"John","last_name":"Smith","email":"test", "address": {"street":"1 Main Street","city":"Wellington","postcode":1234}}'
```
```json
{
  "id":46895213,
  "email":"test",
  "full_name":"John Smith",
  "created_at":"2025-04-20T08:47:28+12:00",
  "address":{
    "street":"1 Main Street",
    "city":"Wellington",
    "postcode":"1234",
    "country":null
  }
}
```

```bash
curl http://0.0.0.0:3000/
I'm home
```

### Module as a Controller
```ruby {filename=Itsi.rb}
module UserController
  module_function
  def create(req, params)
    req.ok json: User.create(params)
  end
end

controller User
post "/", :create
```

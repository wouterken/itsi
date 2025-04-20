---
title: Schemas
url: /middleware/endpoint/schemas
---

Endpoints also support basic schema enforcement for both request and and response bodies. Endpoint **Schemas** are simply plain-old Ruby hashes, which map named keys to allowed types.

E.g.
```ruby {filename=Itsi.rb}
# This is a valid schema
SimpleSchema = {
  score: Integer
}
```

A few special rules/conventions apply:

* Regexp patterns are used in place of symbol keys to define dynamic parameters that must match a specific pattern (A key of `/.*/` matches any string).
* A `:Boolean` symbol can be used instead of a class to require boolean parameters (as a proxy for the combination of TrueClass and FalseClass)
* `Array` keys are used to define arrays of values.
* Objects can be nested
* A special `_required` key indicates which parameters in the current object are required.
* Arrays must be homogeneous (No support for union types).
* Schema validation *only* supports primitive Ruby types, Date, Time, Datetime, arrays and hashes (no unions or advanced coercion).


## Supported types
```ruby {filename=Itsi.rb}
# Demo schema with all supported types included
SupportedTypesSchema = {
  _required: %i[name age active preferences created_at last_login],
  name: String,               # A user's name
  age: Integer,               # The user's age
  active: :Boolean,           # Whether the user is active
  preferences: {
    _required: %i[theme notifications],
    theme: Symbol,            # Preferred theme (e.g., :dark, :light)
    notifications: :Boolean   # Whether notifications are enabled
  },
  scores: Array[Float],       # A list of user scores
  metadata: {
    _required: %i[signup_date last_purchase],
    signup_date: Date,        # The date the user signed up
    last_purchase: DateTime   # The timestamp of the last purchase
  },
  last_login: Time            # The last login time
}
```

{{< callout >}}
Itsi will check for the presence and structure of all:
* [Schemas](/middleware/schemas)
* [Controller methods](/middleware/controller)

at **boot time** ensuring that you can't be caught out by naming mismatches at runtime, or during a hot-reload.
If Itsi boots successfully, the referenced schemas objects can be found.
{{</ callout >}}


## Request Body Schemas

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

## Response Body Schemas
```ruby {filename=Itsi.rb}
PingSchema = {
  status: String
}

get "/ping" do |req, response_format: PingSchema|
  req.ok json: { status: "ok" }, as: response_format
end
```

---
title: Location
url: /middleware/location
---

The `location` block is the essential building block of Itsi-powered apps.
It allows you to selectively apply middleware based on request structure.
This is similar to a `location` block in NGINX, but with added capabilities—most notably, support for nested location blocks.
{{< callout type="warn" >}}
Location blocks are matched in definition order, and only a single location block applies per request.
This rule is applied recursively: the first top-level match is chosen, then the first matching child, and so on.
{{</ callout >}}

## Examples

```ruby {filename="Itsi.rb"}
# Wildcard routes: matches any path beginning with /admin/
location "/admin/*" do
  auth_basic ...
end
```

```ruby {filename="Itsi.rb"}
# Nested routes and named captures
location "/organizations/:organisation_id" do
  location "/users" do
    location "/:user_id([0-9]+)" do
      rate_limiter requests: 10, seconds: 5, ...
      auth_jwt ...
      ...
    end
  end

  # Shared middleware for multiple exact sub-routes.
  location "/settings", "/options" do
    auth_basic ...
  end
end
```

```ruby {filename="Itsi.rb"}
# Regex route: matches either 'users' or 'organizations'
location /(?:users)|(?:organizations)/ do
  intrusion_protection banned_url_patterns: [/wp-admin/]
  ...
end
```
```ruby {filename="Itsi.rb"}
# Match on non-route options.
# Redirect http requests to https requests
location schemes: ["http"]
  redirect type: :permanent, to: "https://{host}{path_and_query}"
end
```

## Route Matches
Routes have several options for matching:

{{% details title="Exact Match (e.g. `\"/api/users\"`)" closed="true" %}}
Matches the complete request path exactly. No prefix matching is performed.
{{% /details %}}


{{% details title="Wildcard Match (e.g. `\"/api/users/*\"`)" closed="true" %}}
Fuzzily matches any path that with support for wild-card dynamic segments.
{{% /details %}}

{{% details title="Named Captures (e.g. `\"/api/users/:id\"`)" closed="true" %}}
Similar to Wildcard match, but captures dynamic segments by name, using `:name` syntax. Matches are delimited by `/`, and captured values are accessible in logs and handlers.

You can restrict captures to specific character sets, using embedded regular expressions:

```ruby
# Matches numeric user IDs
location "/users/:id([0-9]+)" do
  ...
end
```
{{% /details %}}
{{% details title="Regex Match (e.g. `/api\/(users|organizations)/`)" closed="true" %}}
Full regular expression support for matching complex or variable patterns.
{{% /details %}}

{{% details title="Nested Matches" closed="true" %}}
Nested blocks allow deeper route matching. Matching proceeds recursively:
a top-level match is found first, followed by the first matching child block, and so on.
{{% /details %}}


## Options
Location blocks match can also match on several other request attributes:
* `methods`: An array of HTTP methods to match on. E.g. `%w[GET POST PUT DELETE]`
* `protocols`: An array of protocols to match on. %w[http https]
* `hosts`: An array of hosts to match on. E.g. `%w[example.com www.example.com]`
* `ports`: An array of ports to match on. E.g. `%w[80 443]`
* `extensions`: An array of file extensions to match on. E.g. `%w[html css js]`
* `content_types`: An array of content types to match on. E.g. `%w[text/html application/json]`
* `accepts`: An array of accept headers to match on. E.g. `%w[text/html application/json]`

Pass these to the location block using keyword arguments, e.g.

```ruby
# Redirect all http JSON requests to use https exclusively.
location schemes: ["http"], content_types: ["application/json"]
  redirect type: :permanent, to: "https://{host}{path_and_query}"
end
```

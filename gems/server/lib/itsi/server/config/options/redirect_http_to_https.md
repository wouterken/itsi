---
title: Redirect HTTP to HTTPS
url: /options/redirect_http_to_https
---
This simple option installs a location block to redirect all HTTP traffic to HTTPS.

```ruby {filename=Itsi.rb}
redirect_http_to_https!
```

It is an alias for:
```ruby
location protocols: [:http] do
  redirect \
    to: "https://{host}{path_and_query}", \
    type: "moved_permanently"
end
```

It is subject to ordinary [location](/middleware/location) resolution rules.
To make sure this rule takes precedence, place it *above* other locations in the `Itsi.rb` file

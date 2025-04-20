---
title: Middleware
type: docs
next: faqs/
url: /middleware
prev: options/
cascade:
  type: docs
weight: 2
---

Itsi Middleware stacks are modular in nature.
You can pick and choose **just** the features that make sense for you,
and apply these on a *location-by-location* basis.

{{% details title="What's a location?" closed="false" %}}

> A location in Itsi is similar to a Location in NGINX. It's a logical container for all requests matching some combination of:
* Routes/Route expressions
* Request Methods
* Content Types
* Accept Headers
* File types
* Host/port/scheme.

E.g.

```ruby
location "/admin/*" do

  etag \
    type: 'strong',
    algorithm: 'md5',
    min_body_size: 1024 * 1024
  # ...

  location "/public/images", extensions: %w[jpg png] do
    compress \
      min_size: 1024 * 1024,
      level: 'fastest',
      algorithms: %w[zstd gzip brotli deflate],
      mime_types: %w[all],
      compress_streams: true
    # ...
  end
end
```



When a route matches a location block, it recursively inherits *all* middleware that is defined within outer ancestor blocks.
Where a child and an ancestor define the same middleware, the child's middleware takes precedence.

{{% /details %}}

See [location](/middleware/location) for a detailed description of the `location` function.

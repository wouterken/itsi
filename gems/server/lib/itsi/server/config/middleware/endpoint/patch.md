---
title: Patch
url: /middleware/patch
---

The `patch` middleware is an [endpoint](/middleware/endpoint) restricted to PATCH requests.

Endpoints are light-weight inline middleware that can be used to handle requests without the need for a fully fledged Rack-based application framework.
Endpoints can optionally be directed to a controller, and use request and response schema enforcement.

You can use endpoints and rack-apps simultaneously.
See [endpoint](/middleware/endpoint).

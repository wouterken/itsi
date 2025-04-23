## Rails Static Assets
This example demonstrates how you can combine Rails with Itsi's static asset serving capabilities to offload
static assets from Ruby while still running only a single process.

Try and precompile the assets (doing this in development is fine for testing), and testing the difference in performance
by commenting/uncommenting the static assets block at the top of the Itsi.rb file
and for e.g.

running

```bash
# Name of signed asset may changed based on contents
wrk http://127.0.0.1:3000/assets/controllers/application-3affb389.js
```

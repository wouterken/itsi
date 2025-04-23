## Reverse Proxy (2 apps)
This example allows you to toggle between a Rails and Sinatra application hosted as separate services, using Itsi's reverse proxy feature.

Both can also be hosted using Itsi.

To launch these first, execute:

`(cd rails_subapp && bundle exec itsi --rackup_file config.ru -b http://0.0.0.0:4000)`
`(cd sinatra_subapp && bundle exec itsi --rackup_file config.ru -b http://0.0.0.0:6000)`


Then start the Proxy

`itsi`

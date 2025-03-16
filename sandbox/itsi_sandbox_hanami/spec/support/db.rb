# frozen_string_literal: true

# Tag feature spec examples as `:db`
#
# See support/db/cleaning.rb for how the database is cleaned around these `:db` examples.
RSpec.configure do |config|
  config.define_derived_metadata(type: :feature) do |metadata|
    metadata[:db] = true
  end
end

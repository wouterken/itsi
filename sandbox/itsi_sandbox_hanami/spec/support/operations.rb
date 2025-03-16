# frozen_string_literal: true

require "dry/monads"

RSpec.configure do |config|
  # Provide `Success` and `Failure` for testing operation results
  config.include Dry::Monads[:result]
end

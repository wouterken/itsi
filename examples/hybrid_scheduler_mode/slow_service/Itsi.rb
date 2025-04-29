# frozen_string_literal: true

threads 1
fiber_scheduler true

bind "http://0.0.0.0:3005"

endpoint do |req|
  sleep_length = req.query_params.fetch("sleep", 3).to_i
  sleep sleep_length
  req.ok "It's been #{sleep_length} seconds!"
end

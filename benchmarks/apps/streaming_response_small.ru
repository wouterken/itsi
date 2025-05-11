# frozen_string_literal: true

chunk_small = 10.times.map{|i| "data #{i}"}.join(', ') << "\n"

run(
  proc do |_env|
    [
      200, {'content-type' => 'text/plain'}, lambda do |stream|
        5.times do
          stream << chunk_small
          stream.flush
        end
        stream.close
      end
    ]
  end
)

# frozen_string_literal: true

chunk_large = 100.times.map{|i| "data #{i}"}.join(', ') << "\n"

run(
  proc do |_env|
    [
      200, {'content-type' => 'text/plain'}, lambda do |stream|
        1000.times do
          stream << chunk_large
          stream.flush
        end
        stream.close
      end
    ]
  end
)

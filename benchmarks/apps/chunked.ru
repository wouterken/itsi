run Proc.new { |env|
  body = Enumerator.new do |yielder|
    5.times do |i|
      yielder << "Chunk #{i + 1}\n"
      sleep 0.001
    end
  end

  [
    200,
    { 'Content-Type' => 'text/plain' },
    body
  ]
}

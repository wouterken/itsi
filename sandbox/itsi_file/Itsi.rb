location '/*' do
  location '/api' do
    auth_api_key 'valid_keys' => %w[api], 'token_source' => { 'query' => 'APIKey' }, error_response: {
      code: 401,
      default: 'plaintext',
      plaintext: 'What are you doing here Hombre?',
      json: { message: 'Unauthorized', array: [1,2,3,4] }
    }
  end
end

run lambda { |env|
  [200, {}, "Test"]
}

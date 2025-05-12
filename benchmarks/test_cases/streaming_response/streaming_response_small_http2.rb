# frozen_string_literal: true
requires %i[http2 streaming_body]

app File.open('apps/streaming_response_small.ru')

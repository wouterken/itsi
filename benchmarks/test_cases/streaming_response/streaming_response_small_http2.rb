description 'Streaming Response - Small'

workers 1
threads 1

requires %i[http2 streaming_body]

app File.open('apps/streaming_response_small.ru')

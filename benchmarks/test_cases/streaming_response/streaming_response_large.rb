description 'Streaming Response - Large'

workers 1
threads 1

requires %i[streaming_body]

app File.open('apps/streaming_response_large.ru')

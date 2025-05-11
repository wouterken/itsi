description 'Hello World - HTTP2 - 20 Parallel Requests'

workers 1
threads 1

requires %i[http2]

parallel_requests 20

app File.open('apps/hello_world.ru')

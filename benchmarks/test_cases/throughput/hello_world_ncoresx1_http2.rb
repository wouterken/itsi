description 'Hello World - nprocessors workers - h2'

workers Etc.nprocessors
threads 1

requires %i[http2]

app File.open('apps/hello_world.ru')

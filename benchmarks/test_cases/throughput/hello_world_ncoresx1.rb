description 'Hello World - nprocessors workers'

workers Etc.nprocessors
threads 1

app File.open('apps/hello_world.ru')

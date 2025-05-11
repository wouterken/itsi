description 'Hello World - 10 Threads'

workers 1
threads 10

app File.open('apps/hello_world.ru')

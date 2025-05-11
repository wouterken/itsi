description 'Hello World - 4 Workers, 4 Threads'

workers 4
threads 4

app File.open('apps/hello_world.ru')

description 'Sinatra GET 1x1'

path "/get"

workers 4
threads 4

app File.open('apps/sinatra.ru')

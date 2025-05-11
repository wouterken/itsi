description 'Sinatra GET 1x1'

path "/get"

workers 1
threads 1

app File.open('apps/sinatra.ru')

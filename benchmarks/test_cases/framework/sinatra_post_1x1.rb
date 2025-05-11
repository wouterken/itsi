description 'Sinatra POST 1x1'

method 'POST'
path "/post"
data %{{"some":"json"}}

workers 1
threads 1

app File.open('apps/sinatra.ru')

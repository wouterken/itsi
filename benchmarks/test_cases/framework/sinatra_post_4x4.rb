description 'Sinatra POST 1x1'

method 'POST'
path "/post"
data %{{"some":"json"}}

workers 4
threads 4

app File.open('apps/sinatra.ru')

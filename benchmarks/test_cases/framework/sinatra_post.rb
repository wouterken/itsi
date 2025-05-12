method 'POST'
path "/post"
data %{{"some":"json"}}

app File.open('apps/sinatra.ru')

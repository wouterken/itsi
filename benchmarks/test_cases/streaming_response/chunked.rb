requires %i[streaming_body]

nonblocking true

app File.open('apps/chunked.ru')

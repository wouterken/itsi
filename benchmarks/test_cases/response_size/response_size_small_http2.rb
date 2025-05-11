description 'Response Small - h2'

workers 1
threads 1

requires %i[http2]

app File.open('apps/small.ru')

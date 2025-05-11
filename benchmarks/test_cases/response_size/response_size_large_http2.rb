# frozen_string_literal: true
description 'Response Large - h2'

workers 1
threads 1

requires %i[http2]

app File.open('apps/large.ru')

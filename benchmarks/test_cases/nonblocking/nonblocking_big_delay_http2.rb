# frozen_string_literal: true
description 'Non Blocking - Big Delay - H2'

workers 1
threads 1

requires %i[http2]

nonblocking true

app File.open('apps/big_delay.ru')

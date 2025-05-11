# frozen_string_literal: true
description 'Non Blocking - Many Small Delay - H2'

workers 1
threads 1

requires %i[http2]

nonblocking true

app File.open('apps/many_small_delay.ru')

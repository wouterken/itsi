# frozen_string_literal: true
description 'Non Blocking - Many Small Delay'

workers 1
threads 1

nonblocking true

app File.open('apps/many_small_delay.ru')

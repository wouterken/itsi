# frozen_string_literal: true
description 'Non Blocking - Big Delay'

workers 1
threads 1

nonblocking true

app File.open('apps/big_delay.ru')

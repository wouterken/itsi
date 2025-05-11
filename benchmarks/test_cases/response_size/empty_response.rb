# frozen_string_literal: true
description 'Empty Response'

workers 1
threads 1

app File.open('apps/empty.ru')

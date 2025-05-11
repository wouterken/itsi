# frozen_string_literal: true
description 'Response Large'

workers 1
threads 1

app File.open('apps/large.ru')

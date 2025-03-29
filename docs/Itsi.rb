bind 'http://0.0.0.0:3000'

workers 1

watch 'Itsi.rb', [%w[bundle exec itsi restart]]
watch '**/**.md', [%w[hugo build]]

location '/' do
  static_assets \
    root_dir: 'public',
    not_found_behavior: { index: '404.html' },
    auto_index: false,
    try_html_extension: true,
    max_file_size_in_memory: 10 * 1024 * 1024,
    max_files_in_memory: 100,
    file_check_interval: 1
end

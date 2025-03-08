# frozen_string_literal: true

require 'bundler/gem_tasks'
require 'minitest/test_task'
require 'debug'
# Ensure the nested gems' `lib` directories are included in the LOAD_PATH
$LOAD_PATH.unshift(File.expand_path('scheduler/lib', __dir__))
$LOAD_PATH.unshift(File.expand_path('server/lib', __dir__))

GEMS = [
  {
    name: 'itsi-scheduler',
    dir: 'gems/scheduler', # subfolder that holds gem code
    gemspec: 'itsi-scheduler.gemspec',
    rust_name: 'itsi_scheduler' # name of the ext subfolder
  },
  {
    name: 'itsi-server',
    dir: 'gems/server',
    gemspec: 'itsi-server.gemspec',
    rust_name: 'itsi_server'
  }
]

Minitest::TestTask.create do |t|
  t.test_globs = ['gems/**/test/**/*.rb']
  t.warning = true
  t.verbose = true
end

namespace :scheduler do
  desc 'Run tasks in the scheduler directory'
  task :default do
    sh 'cd gems/scheduler && rake'
  end

  task :compile do
    sh 'cd gems/scheduler && rake compile'
  end
end

namespace :server do
  desc 'Run tasks in the server directory'
  task :default do
    sh 'cd gems/server && rake'
  end

  task :compile do
    sh 'cd gems/server && rake compile'
  end
end

desc 'Compile in both scheduler and server directories'
task :compile do
  Rake::Task['scheduler:compile'].invoke
  Rake::Task['server:compile'].invoke
end

Rake::Task[:compile].enhance([:sync_crates])
Rake::Task[:build].enhance([:build_all])

task :sync_crates do
  require 'fileutils'

  GEMS.each do |gem_info|
    Dir.chdir('crates') do
      to_sync = Dir['*'].select do |fn|
        rust_name = fn.split('/', 2).last
        rust_name == gem_info[:rust_name] || GEMS.none? { |g| g[:rust_name] == rust_name }
      end.each do |to_sync|
        system("rsync -q -av #{to_sync}/ ../#{gem_info[:dir]}/ext/#{to_sync} --delete")
      end
    end
  end
end

task :build_all do
  Rake::Task[:sync_crates].invoke
  GEMS.each do |gem_info|
    Dir.chdir(gem_info[:dir]) do
      system("rake build #{gem_info[:gemspec]}") or raise 'Gem build failed'
      built_gem = Dir['pkg/*.gem'].first
      FileUtils.mkdir_p('../../pkg')
      FileUtils.mv(built_gem, "../../#{built_gem}")
    end
  end
end

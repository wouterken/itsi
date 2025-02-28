# frozen_string_literal: true
require "bundler/gem_tasks"
require "minitest/test_task"

# Ensure the nested gems' `lib` directories are included in the LOAD_PATH
$LOAD_PATH.unshift(File.expand_path("scheduler/lib", __dir__))
$LOAD_PATH.unshift(File.expand_path("server/lib", __dir__))

Minitest::TestTask.create do |t|
  t.test_globs = ["**/test/**/*.rb"]
  t.warning = true
  t.verbose = true
end

namespace :scheduler do
  desc "Run tasks in the scheduler directory"
  task :default do
    sh "cd scheduler && rake"
  end

  task :compile do
    sh "cd scheduler && rake compile"
  end
end

namespace :server do
  desc "Run tasks in the server directory"
  task :default do
    sh "cd server && rake"
  end

  task :compile do
    sh "cd server && rake compile"
  end
end

desc "Compile in both scheduler and server directories"
task :compile do
  Rake::Task["scheduler:compile"].invoke
  Rake::Task["server:compile"].invoke
end

# frozen_string_literal: true

require 'bundler/gem_tasks'
require 'minitest/test_task'

# Ensure the nested gems' `lib` directories are included in the LOAD_PATH
$LOAD_PATH.unshift(File.expand_path('scheduler/lib', __dir__))
$LOAD_PATH.unshift(File.expand_path('server/lib', __dir__))

GEMS = [
  {
    shortname: :scheduler,
    name: 'itsi-scheduler',
    dir: 'gems/scheduler', # subfolder that holds gem code
    gemspec: 'itsi-scheduler.gemspec',
    rust_name: 'itsi_scheduler' # name of the ext subfolder
  },
  {
    shortname: :server,
    name: 'itsi-server',
    dir: 'gems/server',
    gemspec: 'itsi-server.gemspec',
    rust_name: 'itsi_server'
  }
]
SHARED_TASKS = %i[compile compile:dev test]

GEMS.each do |gem|
  namespace gem[:shortname] do
    desc "Run tasks in the #{gem[:dir]} directory"
    task :default do
      sh "cd #{gem[:dir]} && bundle exec rake"
    end

    SHARED_TASKS.each do |task|
      task task do
        Rake::Task[:sync_crates].invoke
        sh "cd #{gem[:dir]} && bundle exec rake #{task}"
      end
    end
  end
end

SHARED_TASKS.each do |task|
  desc "#{task} in all Gem directories"
  task task do
    GEMS.each do |gem|
      Rake::Task["#{gem[:shortname]}:#{task}"].invoke
    end
  end
  Rake::Task[task].enhance([:sync_crates])
end

Rake::Task[:build].enhance([:build_all])

task :sync_crates do
  require 'fileutils'
  GEMS.each do |gem_info|
    ext_dir = File.join(gem_info[:dir], 'ext')
    FileUtils.mkdir_p(ext_dir)

    Dir.chdir('crates') do
      Dir['*'].each do |to_sync|
        next unless File.directory?(to_sync)

        dest = File.join('..', gem_info[:dir], 'ext', to_sync)
        system("rsync -q -av #{to_sync}/ #{dest} --delete")
        system("cp ../Cargo.lock ../#{gem_info[:dir]}/Cargo.lock")
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

task :test_env_up do
  system('terraform -chdir=sandbox/deploy apply')
end

task :test_env_down do
  system('terraform -chdir=sandbox/deploy destroy')
end
%i[itsi puma iodine falcon unicorn].each do |server|
  namespace server do
    %i[hanami roda async rack rack_lint rails sinatra].each do |sandbox|
      namespace sandbox do
        task :serve do |args|
          system("(cd sandbox/itsi_sandbox_#{sandbox} && bundle exec #{server} #{ARGV[2..]&.join(' ')} )")
        rescue Interrupt
          # Suppress the stacktrace and message for Interrupt
        end
      end
    end
  end
end

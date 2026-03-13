#!/usr/bin/env ruby
# frozen_string_literal: true

require "pathname"
require "rubygems"

ROOT = Pathname(__dir__).join("..", "..").expand_path

def replace_file(path)
  content = path.read
  updated = yield(content)
  return if updated == content

  path.write(updated)
end

version = ARGV.fetch(0)
Gem::Version.new(version)

replace_file(ROOT.join("lib/itsi/version.rb")) do |content|
  content.sub(/VERSION = ".*?"/, %(VERSION = "#{version}"))
end

replace_file(ROOT.join("gems/server/lib/itsi/server/version.rb")) do |content|
  content.sub(/VERSION = ".*?"/, %(VERSION = "#{version}"))
end

replace_file(ROOT.join("gems/scheduler/lib/itsi/scheduler/version.rb")) do |content|
  content.sub(/VERSION = ".*?"/, %(VERSION = "#{version}"))
end

replace_file(ROOT.join("itsi.gemspec")) do |content|
  content
    .sub(/itsi-scheduler', '= .*?'/, "itsi-scheduler', '= #{version}'")
    .sub(/itsi-server', '= .*?'/, "itsi-server', '= #{version}'")
end

[
  ROOT.join("crates/itsi_server/Cargo.toml"),
  ROOT.join("crates/itsi_scheduler/Cargo.toml")
].each do |path|
  replace_file(path) do |content|
    content.sub(/^version = ".*?"$/, %(version = "#{version}"))
  end
end

puts "Set CI workspace version to #{version}"

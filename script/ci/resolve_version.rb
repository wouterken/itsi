#!/usr/bin/env ruby
# frozen_string_literal: true

require "rubygems"

VERSION_PATTERN = /
  v?
  (
    \d+\.\d+\.\d+
    (?:[.-][0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?
  )
/x

def extract_version(value)
  return if value.nil? || value.strip.empty?

  match = value.match(VERSION_PATTERN)
  match && match[1]
end

version = extract_version(ARGV.first)
version ||= [
  ENV["ITSI_BUILD_VERSION"],
  ENV["GITHUB_HEAD_REF"],
  ENV["GITHUB_REF_NAME"],
  ENV["GITHUB_REF"]
].lazy.map { |value| extract_version(value) }.find(&:itself)

abort("Could not determine a build version. Pass one explicitly or run from a branch/tag that contains a version.") unless version

Gem::Version.new(version)
puts version

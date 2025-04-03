# frozen_string_literal: true

require_relative "lib/itsi/server/version"

Gem::Specification.new do |spec|
  spec.name = "itsi-server"
  spec.version = Itsi::Server::VERSION
  spec.authors = ["Wouter Coppieters"]
  spec.email = ["wc@pico.net.nz"]

  spec.summary = "Itsi Server - A light-weight Rack Server implementation for Ruby."
  spec.description = "Itsi Server - A light-weight Rack Server implementation for Ruby"
  spec.homepage = "https://itsi.fyi"
  spec.license = "MIT"
  spec.required_ruby_version = ">= 2.7.0"
  spec.required_rubygems_version = ">= 3.1"

  spec.metadata["homepage_uri"] = spec.homepage
  spec.metadata["source_code_uri"] = "https://github.com/wouterken/itsi/server"
  spec.metadata["changelog_uri"] = "https://github.com/wouterken/itsi/server/blob/main/CHANGELOG.md"

  # Specify which files should be added to the gem when it is released.
  # The `git ls-files -z` loads the files in the RubyGem that have been added into git.
  gemspec = File.basename(__FILE__)
  spec.files = IO.popen(%w[git ls-files -z], chdir: __dir__, err: IO::NULL) do |ls|
    ls.readlines("\x0", chomp: true).reject do |f|
      (f == gemspec) ||
        f.start_with?(*%w[bin/ test/ spec/ features/ .git appveyor Gemfile])
    end
  end + Dir["../../crates/**/*.{toml,rs,lock,html}"].map do |ext_file|
    "ext/#{ext_file[%r{.*crates/(.*?)$}, 1]}"
  end.compact

  spec.bindir = "exe"
  spec.executables = spec.files.grep(%r{\Aexe/}) { |f| File.basename(f) }
  spec.require_paths = ["lib"]
  spec.extensions = ["ext/itsi_server/extconf.rb"]

  # Uncomment to register a new dependency of your gem
  # spec.add_dependency "example-gem", "~> 1.0"
  spec.add_dependency "rack", ">= 1.6"
  spec.add_dependency "rb_sys", "~> 0.9.91"
  spec.add_development_dependency "ruby-lsp"
  # For more information and examples about making a new gem, check out our
  # guide at: https://bundler.io/guides/creating_gem.html
end

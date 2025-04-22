# frozen_string_literal: true

require_relative 'lib/itsi/version'

Gem::Specification.new do |spec|
  spec.name = 'itsi'
  spec.version = Itsi::VERSION
  spec.authors = ['Wouter Coppieters']
  spec.email = ['wc@pico.net.nz']

  spec.summary = 'Wrapper Gem for both the Itsi server and the Itsi Fiber scheduler'
  spec.description = 'Wrapper Gem for both the Itsi server and the Itsi Fiber scheduler'
  spec.homepage = 'https://itsi.fyi'
  spec.license = 'MIT'
  spec.required_ruby_version = '>= 2.7'
  spec.required_rubygems_version = '>= 3.2'

  spec.metadata['homepage_uri'] = spec.homepage
  spec.metadata['source_code_uri'] = 'https://github.com/wouterken/itsi'
  spec.metadata['changelog_uri'] = 'https://github.com/wouterken/itsi/blob/main/CHANGELOG.md'

  # Specify which files should be added to the gem when it is released.
  # The `git ls-files -z` loads the files in the RubyGem that have been added into git.
  gemspec = File.basename(__FILE__)
  spec.files = IO.popen(%w[git ls-files -z], chdir: __dir__, err: IO::NULL) do |ls|
    ls.readlines("\x0", chomp: true).reject do |f|
      (f == gemspec) ||
        f.start_with?(*%w[bin/ test/ spec/ features/ .git appveyor Gemfile])
    end
  end
  spec.bindir = 'exe'
  spec.executables = spec.files.grep(%r{\Aexe/}) { |f| File.basename(f) }

  spec.require_paths = ['lib']

  spec.add_dependency 'itsi-scheduler', '~> 0.2.3'
  spec.add_dependency 'itsi-server', '~> 0.2.3'

  # For more information and examples about making a new gem, check out our
  # guide at: https://bundler.io/guides/creating_gem.html
end

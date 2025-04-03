#!/usr/bin/env ruby

if ARGV.include?('--install-completions')
  # Try to get the next argument as the shell name.
  shell = ARGV[ARGV.index('--install-completions') + 1] rescue nil
  # Fallback: use the basename of the SHELL environment variable
  shell ||= File.basename(ENV['SHELL'] || '')

  case shell.downcase
  when 'bash'
    # Bash completion script
    puts 'complete -W "$(itsi --completion)" itsi'
  when 'zsh'
    puts <<~ZSH
      _itsi() {
        local completions
        completions=("${(@f)$(itsi --completion)}")
        compadd -- $completions
      }
      compdef _itsi itsi
    ZSH
  else
    warn "Unsupported shell: #{shell}. Please specify 'bash' or 'zsh'."
  end
  exit
end

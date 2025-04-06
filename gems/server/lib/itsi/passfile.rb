module Itsi
  class Server

    module Passfile
      require 'io/console'

      module_function

      def load(filename)
        if filename.nil? || filename.strip.empty?
          puts "Error: a valid filename is required."
          return nil
        end

        creds = {}
        if File.exist?(filename)
          File.foreach(filename) do |line|
            line.chomp!
            next if line.empty?

            user, pass = line.split(':', 2)
            creds[user] = pass
          end
        end
        creds
      end

      def save(creds, filename)
        File.open(filename, 'w', 0o600) do |f|
          creds.each do |u, p|
            f.puts "#{u}:#{p}"
          end
        end
      end

      def echo(filename, algorithm)
        return unless (creds = load(filename))
        print "Enter username: "
        username = $stdin.gets.chomp

        print "Enter password: "

        password = $stdin.noecho(&:gets).chomp
        puts

        print "Confirm password: "
        password_confirm = $stdin.noecho(&:gets).chomp
        puts

        if password != password_confirm
          puts "Error: Passwords do not match!"
          exit(1)
        end

        puts "#{username}:#{Itsi.create_password_hash(password, algorithm)}"
      end

      def add(filename, algorithm)
        return unless (creds = load(filename))
        print "Enter username: "
        username = $stdin.gets.chomp

        print "Enter password: "

        password = $stdin.noecho(&:gets).chomp
        puts

        print "Confirm password: "
        password_confirm = $stdin.noecho(&:gets).chomp
        puts

        if password != password_confirm
          puts "Error: Passwords do not match!"
          exit(1)
        end

        creds[username] = Itsi.create_password_hash(password, algorithm)

        save(creds, filename)

        puts "User '#{username}' added."
      end

      def remove(filename)
        return unless (creds = load(filename))

        print "Enter username to remove: "
        username = $stdin.gets.chomp

        if creds.key?(username)
          creds.delete(username)
          save(creds, filename)
          puts "User '#{username}' removed."
        else
          puts "Warning: User '#{username}' not found."
        end
      end

      def list(filename)
        puts "Current credentials in '#{filename}':"
        return unless (creds = load(filename))
        creds.each do |u, p|
          puts "#{u}:#{p}"
        end
      end

    end
  end
end

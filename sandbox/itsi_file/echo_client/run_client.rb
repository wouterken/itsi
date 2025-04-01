#!/usr/bin/env ruby
# frozen_string_literal: true

require_relative 'echo_client'
require 'optparse'
require 'colorize'

options = {
  host: 'localhost',
  port: 3000,
  compression: nil
}

OptionParser.new do |opts|
  opts.banner = 'Usage: run_client.rb [options]'

  opts.on('-h', '--host HOST', 'Server hostname') do |host|
    options[:host] = host
  end

  opts.on('-p', '--port PORT', 'Server port') do |port|
    options[:port] = port.to_i
  end

  opts.on('-c', '--compression COMPRESSION', 'Compression (none, gzip, deflate)') do |comp|
    options[:compression] = comp.to_sym unless comp == 'none'
  end

  opts.on('-m', '--message MESSAGE', 'Message to send') do |msg|
    options[:message] = msg
  end

  opts.on('--help', 'Show this help message') do
    puts opts
    exit
  end
end.parse!

# Set default message if not provided
options[:message] ||= 'Hello from configurable Ruby client!'

puts "Connecting to #{options[:host]}:#{options[:port]}".yellow
puts "Compression: #{options[:compression] || 'none'}".yellow
puts "Message: #{options[:message]}".yellow

client = EchoClient.new(options[:host], options[:port], compression: options[:compression])

begin
  # # Unary call
  client.echo(options[:message])

  # # Server streaming
  client.echo_stream(options[:message])

  # Client streaming
  client.echo_collect([options[:message], "#{options[:message]} 2", "#{options[:message]} 3"])

  # Bidirectional streaming
  client.echo_bidirectional([options[:message], "#{options[:message]} B", "#{options[:message]} C"])
rescue GRPC::BadStatus => e
  puts "Error from server: #{e.message}".red
ensure
  client.close
end

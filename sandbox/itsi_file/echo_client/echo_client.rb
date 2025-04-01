#!/usr/bin/env ruby
# frozen_string_literal: true

$LOAD_PATH.unshift(File.dirname(__FILE__))


require 'bundler/setup'
require 'grpc'
require 'colorize'
require_relative 'lib/echo_pb'
require_relative 'lib/echo_services_pb'

class EchoClient
  # Initialize the client with options to control compression
  def initialize(host = 'localhost', port = 50051, compression: nil)
    # Available compression options:
    # nil - No compression (default)
    # :gzip - Use gzip compression
    # :deflate - Use deflate compression
    @host = host
    @port = port
    @compression = compression
    
    # Set up the channel and stub with compression options
    channel_args = {}
    if @compression
      # Adding compression options to channel
      channel_args = {
        'grpc.default_compression_algorithm' => compression_algorithm,
        'grpc.default_compression_level' => 2  # High compression level (0-2)
      }
    end
    
    @channel = GRPC::Core::Channel.new("#{host}:#{port}", channel_args, :this_channel_is_insecure)
    @stub = Echo::EchoService::Stub.new("#{host}:#{port}", :this_channel_is_insecure, channel_args: channel_args)
  end
  
  # Map compression symbol to GRPC compression algorithm constant
  def compression_algorithm
    case @compression
    when :gzip
      1  # GRPC::Core::CompressionAlgorithm::GZIP
    when :deflate
      2  # GRPC::Core::CompressionAlgorithm::DEFLATE
    else
      0  # GRPC::Core::CompressionAlgorithm::NONE
    end
  end

  # Simple unary RPC
  def echo(message)
    puts "Calling echo with #{message.inspect}".blue
    request = Echo::EchoRequest.new(message: message)
    response = @stub.echo(request)
    puts "Response: #{response.message.inspect}, Count: #{response.count}".green
    response
  end

  # Server streaming RPC
  def echo_stream(message)
    puts "Calling echo_stream with #{message.inspect}".blue
    request = Echo::EchoRequest.new(message: message)
    responses = []
    
    @stub.echo_stream(request).each do |response|
      puts "Stream Response: #{response.message.inspect}, Count: #{response.count}".green
      responses << response
    end
    
    responses
  end

  # Client streaming RPC
  def echo_collect(messages)
    puts "Calling echo_collect with #{messages.length} messages".blue
    
    # Create a request enumerator
    request_enum = Enumerator.new do |yielder|
      messages.each do |message|
        puts "Sending: #{message.inspect}".cyan
        yielder << Echo::EchoRequest.new(message: message)
      end
    end
    
    # Call with the enumerator
    response = @stub.echo_collect(request_enum)
    
    puts "Collect Response: #{response.message.inspect}, Count: #{response.count}".green
    response
  end

  # Bidirectional streaming RPC
  def echo_bidirectional(messages)
    puts "Calling echo_bidirectional with #{messages.length} messages".blue
    responses = []
    
    # Create a request enumerator
    request_enum = Enumerator.new do |yielder|
      messages.each do |message|
        puts "Sending: #{message.inspect}".cyan
        yielder << Echo::EchoRequest.new(message: message)
      end
    end
    
    # Call bidirectional with enumerator and collect responses
    @stub.echo_bidirectional(request_enum).each do |response|
      puts "Bidi Response: #{response.message.inspect}, Count: #{response.count}".green
      responses << response
    end
    
    responses
  end
  
  # Get the current compression configuration
  def compression_info
    if @compression
      "Using #{@compression} compression (algorithm: #{compression_algorithm}, level: 2)"
    else
      "No compression"
    end
  end
  
  def close
    @channel.close
  end
end

# Only execute if this file is run directly
if __FILE__ == $PROGRAM_NAME
  # Usage example
  compression = ARGV[0]&.to_sym if ARGV[0]
  
  # Show compression options if requested
  if compression == :help
    puts "Available compression options:".yellow
    puts "  none     - No compression (default)"
    puts "  gzip     - Use gzip compression"
    puts "  deflate  - Use deflate compression"
    puts "\nUsage: ruby echo_client.rb [compression]"
    exit
  end
  
  client = EchoClient.new('localhost', 50051, compression: compression)
  puts "Compression: #{client.compression_info}".yellow
  
  begin
    # Unary call
    client.echo("Hello from Ruby client!")
    
    # Server streaming
    client.echo_stream("Stream me back!")
    
    # Client streaming
    client.echo_collect(["Message 1", "Message 2", "Message 3"])
    
    # Bidirectional streaming
    client.echo_bidirectional(["Bidi 1", "Bidi 2", "Bidi 3"])
    
  rescue GRPC::BadStatus => e
    puts "Error from server: #{e.message}".red
  ensure
    client.close
  end
end 
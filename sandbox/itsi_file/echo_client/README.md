# Echo gRPC Client

A simple Ruby gRPC client for the Echo service.

## Setup

1. Install dependencies:

```bash
bundle install
```

2. Generate Ruby code from the proto file:

```bash
./gen_proto.sh
```

## Usage

Run the client with optional compression:

```bash
# Run without compression (default)
ruby echo_client.rb

# Run with gzip compression
ruby echo_client.rb gzip

# Run with deflate compression
ruby echo_client.rb deflate

# Show help
ruby echo_client.rb help
```

## Client Details

This client implements all four types of gRPC methods:

1. Unary RPC (`echo`)
2. Server streaming RPC (`echo_stream`)
3. Client streaming RPC (`echo_collect`)
4. Bidirectional streaming RPC (`echo_bidirectional`)

## Compression Options

You can control compression by modifying the client initialization in `echo_client.rb`:

```ruby
# No compression
client = EchoClient.new('localhost', 50051, compression: nil)

# Gzip compression
client = EchoClient.new('localhost', 50051, compression: :gzip)

# Deflate compression
client = EchoClient.new('localhost', 50051, compression: :deflate)
```

The compression settings are implemented using the gRPC channel arguments, which allows you to test different compression algorithms.

## Troubleshooting

If you encounter any issues:

1. Make sure you have installed all dependencies with `bundle install`
2. Ensure the protocol buffer code has been generated with `./gen_proto.sh`
3. Check that the gRPC server is running on the specified host and port
4. If you see connectivity issues, try running with debugging enabled:

```bash
GRPC_VERBOSITY=DEBUG GRPC_TRACE=all ruby echo_client.rb
```

## Example Server

The client is designed to work with the EchoService server in the `../echo_service_nonitsi` directory. Make sure the server is running before starting the client.

## Compression Testing

To test different compression algorithms with the same message payload:

```bash
# Test without compression
./run_client.rb -m "Large message repeated many times" -c none

# Test with gzip compression
./run_client.rb -m "Large message repeated many times" -c gzip

# Test with deflate compression
./run_client.rb -m "Large message repeated many times" -c deflate
```

This allows you to compare the performance and behavior with different compression settings. 
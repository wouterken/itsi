#!/bin/bash

# Create directory for generated code
mkdir -p lib

# Path to the proto file
PROTO_FILE="../echo.proto"
PROTO_DIR="$(dirname "$PROTO_FILE")"

# Generate Ruby code from proto file
grpc_tools_ruby_protoc \
  --ruby_out=./lib \
  --grpc_out=./lib \
  --proto_path="$PROTO_DIR" \
  "$PROTO_FILE"

echo "Ruby gRPC code generated successfully!" 
#!/bin/bash

# This script tests the echo client with different compression settings

# Message to use for testing
MESSAGE="This is a test message that will be repeated multiple times to demonstrate compression. $(printf '%s' {1..10})"

echo "=== Testing with NO compression ==="
./run_client.rb -m "$MESSAGE" -c none

echo ""
echo "=== Testing with GZIP compression ==="
./run_client.rb -m "$MESSAGE" -c gzip

echo ""
echo "=== Testing with DEFLATE compression ==="
./run_client.rb -m "$MESSAGE" -c deflate

echo ""
echo "All compression tests completed." 
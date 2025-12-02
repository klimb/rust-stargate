#!/bin/sh
# Simple test script for relative path execution in Stargate shell
# Usage: ./simple_test.sh [message]

MESSAGE="${1:-Hello from relative path script}"
echo "$MESSAGE"
echo "Script: $0"
echo "PWD: $(pwd)"

#!/bin/bash
echo "Testing interactive single command (should be human-readable):"
echo "get-environment" | ./target/debug/stargate-shell 2>&1 | head -5
echo ""
echo "Testing interactive pipeline (final should be human-readable):"
echo "get-environment | slice-object count" | ./target/debug/stargate-shell 2>&1

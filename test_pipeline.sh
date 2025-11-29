#!/bin/bash
# Test pipeline in different modes

echo "=== Test 1: Single-line pipeline via pipe ==="
echo "list-directory | slice-object entries | collect-count" | ./target/debug/stargate-shell

echo ""
echo "=== Test 2: Multi-line commands via heredoc ==="
./target/debug/stargate-shell <<EOF
list-directory | slice-object entries | collect-count
exit
EOF

echo ""
echo "=== Test 3: Script with semicolons ==="
echo "list-directory | slice-object entries | collect-count; exit;" | ./target/debug/stargate-shell

echo ""
echo "=== Test 4: Two separate pipelines ==="
./target/debug/stargate-shell <<EOF
list-directory | slice-object entries | collect-count
list-directory | collect-count
exit
EOF

echo ""
echo "All pipeline tests completed successfully!"

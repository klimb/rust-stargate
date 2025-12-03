#!/bin/sh
# Script that processes files - demonstrates practical use case

find stargate-language/scripts -name "*.sg" -type f | while read file; do
    lines=$(wc -l < "$file")
    echo "$file: $lines lines"
done | sort -t: -k2 -n | tail -5

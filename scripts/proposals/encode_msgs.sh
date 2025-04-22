#!/bin/bash
set -e

# Encode each message JSON to base64 (no line breaks)
for i in 1 2 3; do
  base64 -b 0 -i "scripts/proposals/msg${i}.json" > "scripts/proposals/msg${i}.b64"
done
echo "All messages encoded to base64."
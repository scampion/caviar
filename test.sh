#!/bin/bash

# Build the server
cargo build

# Run the server in the background
./target/debug/piiserver &
SERVER_PID=$!

# Wait for the server to start (adjust sleep time as needed)
sleep 2

# Test the API with curl
curl -X POST \
  -H "Content-Type: application/json" \
  -d '{"text": "My email is john.doe@example.com and my phone number is 555-123-4567."}' \
  http://localhost:8080/detect_pii

sleep 2

# Test the API with curl
curl -X POST \
  -H "Content-Type: application/json" \
  -d '{"text": "My email is john.doe@example.com and my phone number is 555-123-4567."}' \
  http://localhost:8080/detect_pii


# Test pdf
curl -X POST http://localhost:8080/detect_and_replace_pii_pdf -H "Content-Type: application/pdf" --data-binary @test.pdf --output sanitized.pdf


# Kill the server
kill $SERVER_PID

# Exit with success
exit 0

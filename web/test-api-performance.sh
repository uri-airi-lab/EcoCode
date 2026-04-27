#!/usr/bin/env bash
# Test API performance after fixes

BACKEND_URL="https://backend.blacktree-5d3c8de6.eastus.azurecontainerapps.io"

echo "Testing API Performance..."
echo "========================="

# Test basic endpoints
echo "1. Testing /api/hello"
time curl -s "$BACKEND_URL/api/hello" > /dev/null
echo

echo "2. Testing /api/projects (might need auth)"
time curl -s "$BACKEND_URL/api/projects" > /dev/null
echo

echo "3. Testing /api/runs (might need auth)"  
time curl -s "$BACKEND_URL/api/runs" > /dev/null
echo

echo "4. Testing SSE connection (5 second timeout)"
timeout 5 curl -s "$BACKEND_URL/api/sse" > /dev/null || echo "SSE timeout (expected)"
echo

echo "Performance test complete!"
echo "If times are under 5 seconds, performance should be good."
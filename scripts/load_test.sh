#!/bin/bash

# Make script executable
chmod +x load_test.sh

# Function to measure response time
measure_request() {
    local url=$1
    local description=$2
    echo "Testing: $description"
    time curl -s "$url" > /dev/null
    echo "----------------------------------------"
}

# Base URL
BASE_URL="http://localhost:8081"

# Test different scenarios
echo "Starting load tests..."

# Test 1: List only (no images)
measure_request "$BASE_URL/images?page=1&per_page=100" "List only (100 items)"

# Test 2: With thumbnails
measure_request "$BASE_URL/images?page=1&per_page=100&include_thumbnail=true" "With thumbnails (100 items)"

# Test 3: With full images
measure_request "$BASE_URL/images?page=1&per_page=100&include_data=true" "With full images (100 items)"

# Test 4: With both thumbnails and full images
measure_request "$BASE_URL/images?page=1&per_page=100&include_thumbnail=true&include_data=true" "With both (100 items)"

# Memory usage
echo "Memory usage:"
ps aux | grep "images-api" | grep -v grep

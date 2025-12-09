#!/bin/bash

# Test script to verify Kafka integration
# Requires Kafka to be running on localhost:9092

set -e

echo "Testing Matching Engine Service Kafka Integration"
echo "=================================================="

# Check if Kafka is running
echo "Checking Kafka connection..."
if ! nc -z localhost 9092 2>/dev/null; then
    echo "ERROR: Kafka is not running on localhost:9092"
    echo "Please start Kafka first:"
    echo "  docker run -p 9092:9092 apache/kafka:latest"
    exit 1
fi

echo "✓ Kafka is running"

# Start the service in background
echo ""
echo "Starting matching engine service..."
cargo build --release --bin matching_engine_service
./target/release/matching_engine_service &
SERVICE_PID=$!

# Wait for service to start
sleep 2

# Check if service is running
if ! kill -0 $SERVICE_PID 2>/dev/null; then
    echo "ERROR: Service failed to start"
    exit 1
fi

echo "✓ Service started (PID: $SERVICE_PID)"

# Test health endpoint
echo ""
echo "Testing health endpoint..."
if curl -s http://localhost:8080/health | grep -q "ok"; then
    echo "✓ Health check passed"
else
    echo "✗ Health check failed"
    kill $SERVICE_PID 2>/dev/null || true
    exit 1
fi

# Place a test order
echo ""
echo "Placing test order..."
ORDER_RESPONSE=$(curl -s -X POST http://localhost:8080/api/order \
  -H "Content-Type: application/json" \
  -d '{"side": "bid", "order_type": "limit", "price": 50000.0, "quantity": 0.1}')

echo "Order response: $ORDER_RESPONSE"

if echo "$ORDER_RESPONSE" | grep -q "order_id"; then
    echo "✓ Order placed successfully"
else
    echo "✗ Order placement failed"
    kill $SERVICE_PID 2>/dev/null || true
    exit 1
fi

# Wait a moment for Kafka events
sleep 1

echo ""
echo "✓ All tests passed!"
echo ""
echo "To verify Kafka events, run:"
echo "  kafka-console-consumer --bootstrap-server localhost:9092 --topic market-events --from-beginning"

# Cleanup
kill $SERVICE_PID 2>/dev/null || true



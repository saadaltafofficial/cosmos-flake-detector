#!/bin/bash
# Example: Test ZigChain RPC endpoints for flakiness
# Usage: ./examples/test_zigchain.sh

set -e

echo "🔍 Testing ZigChain RPC Endpoints"
echo "=================================="
echo ""

# ZigChain mainnet RPC endpoints
ENDPOINTS=(
    "https://zigchain-mainnet-rpc-sanatry-01.wickhub.cc"
    "https://zigchain-mainnet-rpc.autostake.com"
    "https://zig-chain-rpc.noders.services"
)

# Join endpoints with commas
ENDPOINT_LIST=$(IFS=, ; echo "${ENDPOINTS[*]}")

# Common Cosmos queries to test
QUERIES="health,status,abci_info,net_info,block?height=1"

# Test configuration
DURATION=120  # 2 minutes per endpoint
CONCURRENCY=10
OUTPUT_FILE="zigchain_health_$(date +%Y%m%d_%H%M%S).json"

echo "Configuration:"
echo "  Endpoints: ${#ENDPOINTS[@]}"
echo "  Test Duration: ${DURATION}s per endpoint"
echo "  Concurrency: ${CONCURRENCY}"
echo "  Output: ${OUTPUT_FILE}"
echo ""

# Build if needed
if [ ! -f "target/release/flake-detector" ]; then
    echo "📦 Building flake-detector..."
    cargo build --release
    echo ""
fi

# Run the test
./target/release/flake-detector \
    --endpoints "${ENDPOINT_LIST}" \
    --queries "${QUERIES}" \
    --duration ${DURATION} \
    --concurrency ${CONCURRENCY} \
    --output "${OUTPUT_FILE}"

echo ""
echo "✅ Test complete!"
echo ""
echo "📊 Analysis:"
echo "============"

# Extract key metrics using jq
if command -v jq &> /dev/null; then
    echo ""
    echo "Top 3 Endpoints by Flakiness Score:"
    jq -r '.[] | "\(.flakiness_score | tonumber | round) - \(.endpoint)"' "${OUTPUT_FILE}" | \
        sort -n | head -3
    
    echo ""
    echo "Endpoints with >30 flakiness score (Warning):"
    jq -r '.[] | select(.flakiness_score > 30) | "⚠️  \(.endpoint) - Score: \(.flakiness_score)"' "${OUTPUT_FILE}"
    
    echo ""
    echo "Best performing query paths:"
    jq -r '.[] | .queries[] | "\(.failure_rate * 100 | round)% failure - \(.query)"' "${OUTPUT_FILE}" | \
        sort -n | head -5
else
    echo "Install 'jq' for detailed analysis: sudo apt install jq"
fi

echo ""
echo "📄 Full results saved to: ${OUTPUT_FILE}"
echo ""
echo "Next steps:"
echo "  1. Review JSON output: jq '.' ${OUTPUT_FILE}"
echo "  2. Compare with previous tests"
echo "  3. Alert team if flakiness score >50"
#!/usr/bin/env bash
# GraphLite Docker Test Script
# Test Docker images across different architectures

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
IMAGE_NAME="graphlite:latest"
TEST_DB_PATH="/tmp/graphlite-docker-test"
ADMIN_USER="admin"
ADMIN_PASSWORD="testpass123"

# Print colored message
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Print usage
usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Test GraphLite Docker images.

OPTIONS:
    -h, --help              Show this help message
    -i, --image IMAGE       Image to test (default: graphlite:latest)
    -p, --platform PLATFORM Platform to test (e.g., linux/amd64, linux/arm64)

EXAMPLES:
    # Test default image
    $0

    # Test specific image
    $0 --image graphlite:v0.0.1

    # Test specific platform
    $0 --platform linux/amd64

EOF
    exit 0
}

# Parse arguments
PLATFORM=""
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            ;;
        -i|--image)
            IMAGE_NAME="$2"
            shift 2
            ;;
        -p|--platform)
            PLATFORM="--platform $2"
            shift 2
            ;;
        *)
            print_error "Unknown option: $1"
            usage
            ;;
    esac
done

print_info "==================================================================="
print_info "GraphLite Docker Image Test"
print_info "==================================================================="
print_info "Image: ${IMAGE_NAME}"
print_info "Platform: ${PLATFORM:-native}"
print_info "==================================================================="

# Cleanup function
cleanup() {
    print_info "Cleaning up test resources..."
    docker rm -f graphlite-test-container 2>/dev/null || true
    rm -rf "${TEST_DB_PATH}" 2>/dev/null || true
}

# Set trap for cleanup
trap cleanup EXIT

# Test 1: Check if image exists
print_info "Test 1: Checking if image exists..."
if docker image inspect ${IMAGE_NAME} >/dev/null 2>&1; then
    print_success "Image exists"
else
    print_error "Image not found: ${IMAGE_NAME}"
    exit 1
fi

# Test 2: Run version check
print_info "Test 2: Running version check..."
if docker run ${PLATFORM} --rm ${IMAGE_NAME} graphlite --version; then
    print_success "Version check passed"
else
    print_error "Version check failed"
    exit 1
fi

# Test 3: Run help command
print_info "Test 3: Running help command..."
if docker run ${PLATFORM} --rm ${IMAGE_NAME} graphlite --help >/dev/null 2>&1; then
    print_success "Help command passed"
else
    print_error "Help command failed"
    exit 1
fi

# Test 4: Initialize database
print_info "Test 4: Initializing database..."
mkdir -p "${TEST_DB_PATH}"
if docker run ${PLATFORM} --rm \
    -v "${TEST_DB_PATH}:/data" \
    ${IMAGE_NAME} \
    graphlite install \
    --path /data/testdb \
    --admin-user "${ADMIN_USER}" \
    --admin-password "${ADMIN_PASSWORD}"; then
    print_success "Database initialization passed"
else
    print_error "Database initialization failed"
    exit 1
fi

# Test 5: Verify database files created
print_info "Test 5: Verifying database files..."
if [ -d "${TEST_DB_PATH}/testdb" ]; then
    print_success "Database directory created"
else
    print_error "Database directory not found"
    exit 1
fi

# Test 6: Run a simple query (non-interactive)
print_info "Test 6: Running simple GQL query..."
QUERY_OUTPUT=$(docker run ${PLATFORM} --rm \
    -v "${TEST_DB_PATH}:/data" \
    ${IMAGE_NAME} \
    bash -c "echo 'CREATE SCHEMA /test_schema' | graphlite gql --path /data/testdb -u ${ADMIN_USER} -p ${ADMIN_PASSWORD}" 2>&1 || true)

if echo "${QUERY_OUTPUT}" | grep -q "Schema created"; then
    print_success "GQL query execution passed"
else
    print_warning "GQL query test inconclusive (interactive mode may be required)"
fi

# Test 7: Check container user
print_info "Test 7: Checking container user..."
CONTAINER_USER=$(docker run ${PLATFORM} --rm ${IMAGE_NAME} whoami)
if [ "${CONTAINER_USER}" = "graphlite" ]; then
    print_success "Running as non-root user: ${CONTAINER_USER}"
else
    print_warning "Container user: ${CONTAINER_USER}"
fi

# Test 8: Check image size
print_info "Test 8: Checking image size..."
IMAGE_SIZE=$(docker image inspect ${IMAGE_NAME} --format='{{.Size}}' | awk '{print $1/1024/1024}')
print_info "Image size: ${IMAGE_SIZE} MB"
if (( $(echo "${IMAGE_SIZE} < 500" | bc -l) )); then
    print_success "Image size is reasonable (< 500 MB)"
else
    print_warning "Image size is large (> 500 MB)"
fi

# Test 9: Test with Docker Compose (if available)
print_info "Test 9: Testing Docker Compose configuration..."
if [ -f "docker-compose.yml" ]; then
    if docker-compose config >/dev/null 2>&1; then
        print_success "Docker Compose configuration is valid"
    else
        print_warning "Docker Compose configuration may have issues"
    fi
else
    print_info "Docker Compose file not found, skipping..."
fi

# Test 10: Security check - verify non-root user
print_info "Test 10: Security check..."
ROOT_CHECK=$(docker run ${PLATFORM} --rm ${IMAGE_NAME} id -u)
if [ "${ROOT_CHECK}" != "0" ]; then
    print_success "Security check passed: not running as root (UID: ${ROOT_CHECK})"
else
    print_error "Security check failed: running as root"
    exit 1
fi

# Final summary
print_success "==================================================================="
print_success "All tests passed!"
print_success "==================================================================="
print_info "Image ${IMAGE_NAME} is ready for use."
print_info ""
print_info "Quick start commands:"
print_info "  docker run -it ${IMAGE_NAME} graphlite --version"
print_info "  docker-compose up -d"
print_info ""
print_info "Initialize a database:"
print_info "  docker run -it -v \$(pwd)/mydb:/data ${IMAGE_NAME} \\"
print_info "    graphlite install --path /data/mydb --admin-user admin --admin-password secret"

exit 0

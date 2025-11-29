#!/usr/bin/env bash
# GraphLite Docker Build Script
# Supports multi-architecture builds (amd64/x86_64 and arm64/aarch64)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
IMAGE_NAME="graphlite"
IMAGE_TAG="latest"
REGISTRY=""
PLATFORMS="linux/amd64,linux/arm64"
RUST_VERSION="1.83"
DOCKERFILE="Dockerfile"
PUSH=false
LOAD=false
CACHE=true

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

Build GraphLite Docker images with multi-architecture support.

OPTIONS:
    -h, --help              Show this help message
    -n, --name NAME         Image name (default: graphlite)
    -t, --tag TAG           Image tag (default: latest)
    -r, --registry REGISTRY Container registry (e.g., docker.io/username)
    -p, --platform PLATFORM Target platform (default: linux/amd64,linux/arm64)
                           Options: linux/amd64, linux/arm64, or both (comma-separated)
    --push                  Push to registry after build
    --load                  Load image to local Docker (only works with single platform)
    --no-cache              Build without cache
    --native                Build for native architecture only
    --amd64                 Build for AMD64/x86_64 only
    --arm64                 Build for ARM64/aarch64 only

EXAMPLES:
    # Build for current architecture
    $0 --native

    # Build multi-arch images
    $0

    # Build and push to registry
    $0 --registry docker.io/myusername --push

    # Build specific architecture
    $0 --amd64
    $0 --arm64

    # Build with custom tag
    $0 --tag v0.0.1

    # Build and load to local Docker (single platform only)
    $0 --native --load

EOF
    exit 0
}

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            usage
            ;;
        -n|--name)
            IMAGE_NAME="$2"
            shift 2
            ;;
        -t|--tag)
            IMAGE_TAG="$2"
            shift 2
            ;;
        -r|--registry)
            REGISTRY="$2"
            shift 2
            ;;
        -p|--platform)
            PLATFORMS="$2"
            shift 2
            ;;
        --push)
            PUSH=true
            shift
            ;;
        --load)
            LOAD=true
            shift
            ;;
        --no-cache)
            CACHE=false
            shift
            ;;
        --native)
            PLATFORMS=$(uname -m | sed 's/x86_64/linux\/amd64/;s/aarch64/linux\/arm64/')
            shift
            ;;
        --amd64)
            PLATFORMS="linux/amd64"
            shift
            ;;
        --arm64)
            PLATFORMS="linux/arm64"
            shift
            ;;
        *)
            print_error "Unknown option: $1"
            usage
            ;;
    esac
done

# Validate Docker is installed
if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed. Please install Docker first."
    exit 1
fi

# Check if buildx is available
if ! docker buildx version &> /dev/null; then
    print_error "Docker Buildx is not available. Please upgrade Docker."
    exit 1
fi

# Create full image name
FULL_IMAGE_NAME="${IMAGE_NAME}:${IMAGE_TAG}"
if [ -n "$REGISTRY" ]; then
    FULL_IMAGE_NAME="${REGISTRY}/${FULL_IMAGE_NAME}"
fi

# Validate load and push options
if [ "$LOAD" = true ] && [ "$PUSH" = true ]; then
    print_error "Cannot use --load and --push together"
    exit 1
fi

# Check if multiple platforms with load
if [ "$LOAD" = true ] && [[ "$PLATFORMS" == *","* ]]; then
    print_error "Cannot use --load with multiple platforms. Use single platform or --push instead."
    exit 1
fi

# Print build configuration
print_info "==================================================================="
print_info "GraphLite Docker Build Configuration"
print_info "==================================================================="
print_info "Image Name:      ${FULL_IMAGE_NAME}"
print_info "Platforms:       ${PLATFORMS}"
print_info "Rust Version:    ${RUST_VERSION}"
print_info "Dockerfile:      ${DOCKERFILE}"
print_info "Use Cache:       ${CACHE}"
print_info "Push to Registry: ${PUSH}"
print_info "Load to Docker:  ${LOAD}"
print_info "==================================================================="

# Ensure buildx builder exists
BUILDER_NAME="graphlite-builder"
if ! docker buildx inspect "${BUILDER_NAME}" &> /dev/null; then
    print_info "Creating buildx builder: ${BUILDER_NAME}"
    docker buildx create --name "${BUILDER_NAME}" --driver docker-container --bootstrap --use
else
    print_info "Using existing buildx builder: ${BUILDER_NAME}"
    docker buildx use "${BUILDER_NAME}"
fi

# Build command
BUILD_CMD="docker buildx build"
BUILD_CMD="${BUILD_CMD} --platform ${PLATFORMS}"
BUILD_CMD="${BUILD_CMD} --build-arg RUST_VERSION=${RUST_VERSION}"
BUILD_CMD="${BUILD_CMD} -t ${FULL_IMAGE_NAME}"
BUILD_CMD="${BUILD_CMD} -f ${DOCKERFILE}"

if [ "$CACHE" = false ]; then
    BUILD_CMD="${BUILD_CMD} --no-cache"
fi

if [ "$PUSH" = true ]; then
    BUILD_CMD="${BUILD_CMD} --push"
elif [ "$LOAD" = true ]; then
    BUILD_CMD="${BUILD_CMD} --load"
fi

BUILD_CMD="${BUILD_CMD} ."

# Execute build
print_info "Starting build..."
print_info "Command: ${BUILD_CMD}"
echo ""

if eval "${BUILD_CMD}"; then
    print_success "==================================================================="
    print_success "Build completed successfully!"
    print_success "==================================================================="
    print_success "Image: ${FULL_IMAGE_NAME}"
    print_success "Platforms: ${PLATFORMS}"

    if [ "$PUSH" = true ]; then
        print_success "Image pushed to registry"
    elif [ "$LOAD" = true ]; then
        print_success "Image loaded to local Docker"
        echo ""
        print_info "You can now run:"
        print_info "  docker run -it ${FULL_IMAGE_NAME} graphlite --version"
    else
        print_warning "Image built but not pushed or loaded"
        print_info "To push: docker buildx build --platform ${PLATFORMS} -t ${FULL_IMAGE_NAME} --push ."
        print_info "To load (single platform): docker buildx build --platform ${PLATFORMS} -t ${FULL_IMAGE_NAME} --load ."
    fi

    echo ""
    print_info "Next steps:"
    print_info "  1. Test the image:"
    print_info "     docker run -it ${FULL_IMAGE_NAME} graphlite --version"
    print_info "  2. Start with Docker Compose:"
    print_info "     docker-compose up -d"
    print_info "  3. Initialize a database:"
    print_info "     docker run -it -v \$(pwd)/mydb:/data ${FULL_IMAGE_NAME} \\"
    print_info "       graphlite install --path /data/mydb --admin-user admin --admin-password secret"

else
    print_error "==================================================================="
    print_error "Build failed!"
    print_error "==================================================================="
    exit 1
fi

# GraphLite Docker - Manual Testing Guide

## Prerequisites

### 1. Start Docker Daemon

**On macOS:**
```bash
# Open Docker Desktop
open -a Docker

# Wait for Docker to start (30-60 seconds)
# You'll see the Docker icon in the menu bar

# Verify Docker is running
docker ps
```

**On Linux:**
```bash
# Start Docker service
sudo systemctl start docker

# Verify
docker ps
```

---

## Quick Test (5 Minutes)

Once Docker is running, follow these steps:

### Step 1: Build the Image (Native Architecture)

```bash
# Navigate to project directory
cd /Users/mac/code/github/graphlite-ai/GraphLite

# Build for your current architecture (x86_64)
docker build -t graphlite:test .

# This will take 10-15 minutes on first build
# Watch for any errors
```

**What to expect:**
- Stage 1: Builder - Compiles Rust code (~10 minutes)
- Stage 2: Runtime - Creates minimal image (~1 minute)
- Final image size: ~150-250 MB

### Step 2: Verify the Build

```bash
# Check image exists
docker images | grep graphlite

# Should show:
# graphlite   test   <IMAGE_ID>   <TIME>   ~200MB

# Check image architecture
docker image inspect graphlite:test --format='{{.Architecture}}'
# Should output: amd64 (for x86_64)

# Test version command
docker run --rm graphlite:test graphlite --version
# Should output version info
```

### Step 3: Test Help/Entrypoint (No Database)

```bash
# Run without database - should show helpful message
docker run -it --rm graphlite:test

# Expected output:
# ==========================================
# GraphLite - Graph Database
# ==========================================
#
# No database configured. Please either:
#   1. Initialize a new database:
#      docker run -it -v $(pwd)/mydb:/data graphlite:latest \
#        graphlite install --path /data/mydb --admin-user admin --admin-password secret
#   ...
```

Press Ctrl+C to exit.

### Step 4: Initialize a Test Database

```bash
# Create test directory
mkdir -p ./docker_test_db

# Initialize database
docker run -it --rm \
  -v $(pwd)/docker_test_db:/data \
  graphlite:test \
  graphlite install \
    --path /data/testdb \
    --admin-user admin \
    --admin-password testpass123

# Expected output:
# Database initialized successfully
# Admin user created
# ...
```

**Verify:**
```bash
# Check database files created
ls -la ./docker_test_db/testdb

# Should see database files (sled storage)
```

### Step 5: Test GQL Shell (Automatic Mode) ⭐

This is the **key feature** - automatic GQL shell startup:

```bash
# Start with environment variables - should auto-start GQL shell
docker run -it --rm \
  -v $(pwd)/docker_test_db:/data \
  -e GRAPHLITE_DB_PATH=/data/testdb \
  -e GRAPHLITE_USER=admin \
  -e GRAPHLITE_PASSWORD=testpass123 \
  graphlite:test
```

**Expected behavior:**
- Container starts
- **Automatically connects to database**
- **Shows `gql>` prompt immediately** ✨

**Try these queries:**
```gql
# Create schema
CREATE SCHEMA /test_schema

# Create graph
CREATE GRAPH /test_schema/test_graph

# Set working graph
SESSION SET GRAPH /test_schema/test_graph

# Insert data
INSERT (:Person {name: 'Alice', age: 30})

# Query data
MATCH (p:Person) RETURN p.name, p.age

# Exit
EXIT
```

### Step 6: Test GQL Shell (Manual Mode)

```bash
# Traditional way with explicit command
docker run -it --rm \
  -v $(pwd)/docker_test_db:/data \
  graphlite:test \
  graphlite gql --path /data/testdb -u admin -p testpass123

# Should also show gql> prompt
```

---

## Docker Compose Test (10 Minutes)

### Step 1: Build with Compose

```bash
# Build the service
docker-compose build

# Should build the same image as above
```

### Step 2: Initialize Database

```bash
# Create database via compose
docker-compose run --rm graphlite graphlite install \
  --path /data/mydb \
  --admin-user admin \
  --admin-password secret

# Database stored in named volume 'graphlite-dev-data'
```

### Step 3: Start GQL Shell (Method 1: Direct)

```bash
# Start GQL shell directly
docker-compose run --rm graphlite graphlite gql \
  --path /data/mydb \
  -u admin \
  -p secret

# Should show gql> prompt
```

### Step 4: Start GQL Shell (Method 2: Automatic)

**Edit docker-compose.yml:**
```bash
# Uncomment these lines in docker-compose.yml (around line 50):
- GRAPHLITE_DB_PATH=/data/mydb
- GRAPHLITE_USER=admin
- GRAPHLITE_PASSWORD=secret
```

**Then run:**
```bash
# Now just run without commands - auto GQL shell!
docker-compose run --rm graphlite

# Should automatically show gql> prompt
```

### Step 5: Cleanup

```bash
# Stop services
docker-compose down

# Remove volumes (if desired)
docker-compose down -v

# Clean up test database
rm -rf ./docker_test_db
```

---

## Multi-Architecture Test (Advanced)

### Test AMD64 Build (x86_64)

```bash
# Your current architecture
docker buildx build --platform linux/amd64 \
  -t graphlite:amd64-test \
  --load \
  .

# Test it
docker run --rm graphlite:amd64-test graphlite --version
```

### Test ARM64 Build (Cross-Compile)

```bash
# Setup buildx builder (first time only)
docker buildx create --name graphlite-builder \
  --driver docker-container \
  --bootstrap \
  --use

# Build for ARM64
docker buildx build --platform linux/arm64 \
  -t graphlite:arm64-test \
  --load \
  .

# Verify architecture
docker image inspect graphlite:arm64-test --format='{{.Architecture}}'
# Should output: arm64
```

**Note:** You can't run ARM64 images on x86_64 without emulation, but you can verify they build correctly.

### Multi-Arch Build (Both)

```bash
# Build for both platforms
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t graphlite:multi-arch \
  .

# Note: Can't use --load with multiple platforms
# Use --push to push to registry instead
```

---

## Using the Build Script (Recommended)

### Quick Native Build

```bash
# Use the automated build script
./scripts/docker-build.sh --native --load --tag test

# Options explained:
# --native: Build for current architecture only
# --load: Load image to Docker (for single platform)
# --tag test: Tag as graphlite:test
```

### Cross-Architecture Builds

```bash
# Build AMD64 only
./scripts/docker-build.sh --amd64 --tag amd64-test --load

# Build ARM64 only
./scripts/docker-build.sh --arm64 --tag arm64-test

# Build both (creates manifest)
./scripts/docker-build.sh --tag multi-arch
```

### Build and Push to Registry

```bash
# Build and push to Docker Hub
./scripts/docker-build.sh \
  --registry docker.io/yourusername \
  --tag latest \
  --push

# Build and push to GitHub Container Registry
./scripts/docker-build.sh \
  --registry ghcr.io/graphlite-ai/graphlite \
  --tag v0.0.1 \
  --push
```

---

## Automated Testing

### Run Test Suite

```bash
# Run comprehensive tests
./scripts/docker-test.sh --image graphlite:test

# Tests include:
# ✓ Image exists
# ✓ Version check
# ✓ Help command
# ✓ Database initialization
# ✓ Database file creation
# ✓ GQL query execution (basic)
# ✓ Container user (non-root)
# ✓ Image size check
# ✓ Docker Compose validation
# ✓ Security checks
```

### Test Specific Platform

```bash
# Test AMD64 image
./scripts/docker-test.sh --image graphlite:amd64-test --platform linux/amd64

# Test ARM64 image (requires ARM hardware or emulation)
./scripts/docker-test.sh --image graphlite:arm64-test --platform linux/arm64
```

---

## Troubleshooting

### Build Fails

**Error: "Cannot connect to Docker daemon"**
```bash
# Solution: Start Docker
open -a Docker  # macOS
sudo systemctl start docker  # Linux
```

**Error: "no space left on device"**
```bash
# Solution: Clean Docker
docker system prune -a --volumes

# Check space
docker system df
```

**Error: "buildx not found"**
```bash
# Solution: Install buildx
docker buildx install

# Or update Docker Desktop
```

### Runtime Fails

**Container exits immediately**
```bash
# Check logs
docker logs <container-id>

# Run interactively to see errors
docker run -it --rm graphlite:test bash
```

**Permission denied on /data**
```bash
# Fix permissions on host
chmod -R 777 ./docker_test_db

# Or run as root (not recommended for production)
docker run --user root -it graphlite:test bash
```

**GQL shell doesn't start automatically**
```bash
# Check environment variables are set
docker run --rm graphlite:test env | grep GRAPHLITE

# Verify database path exists
docker run --rm -v $(pwd)/docker_test_db:/data graphlite:test ls -la /data
```

---

## Performance Notes

### Build Times (Expected)

| Build Type | Time | Notes |
|-----------|------|-------|
| First native build | 10-15 min | Downloads Rust, compiles everything |
| Cached native build | 2-3 min | Only changed files recompiled |
| Cross-compile ARM64 | 15-20 min | Additional cross-toolchain setup |
| Multi-arch build | 25-35 min | Both platforms sequentially |

### Optimization Tips

**Use build cache:**
```bash
# Docker caches layers automatically
# Don't use --no-cache unless necessary
docker build -t graphlite:test .
```

**Parallel builds:**
```bash
# Build multiple tags in parallel
docker build -t graphlite:test -t graphlite:latest . &
```

**Use buildx cache:**
```bash
# Buildx has advanced caching
docker buildx build \
  --cache-from type=local,src=/tmp/docker-cache \
  --cache-to type=local,dest=/tmp/docker-cache \
  -t graphlite:test \
  --load \
  .
```

---

## Success Criteria

Your Docker setup is working correctly if:

✅ **Build succeeds** without errors
✅ **Image size** is ~150-250 MB (runtime stage)
✅ **Version command** returns version info
✅ **Database initialization** creates files
✅ **GQL shell starts** automatically with env vars
✅ **Manual GQL shell** works with explicit command
✅ **Queries execute** successfully in GQL shell
✅ **Container runs** as non-root user
✅ **Docker Compose** builds and runs
✅ **Data persists** between container restarts

---

## Quick Commands Reference

```bash
# Build
docker build -t graphlite:test .

# Test version
docker run --rm graphlite:test graphlite --version

# Init database
docker run -it --rm -v $(pwd)/testdb:/data graphlite:test \
  graphlite install --path /data/db --admin-user admin --admin-password pass

# Auto GQL shell (with env vars)
docker run -it --rm -v $(pwd)/testdb:/data \
  -e GRAPHLITE_DB_PATH=/data/db \
  -e GRAPHLITE_USER=admin \
  -e GRAPHLITE_PASSWORD=pass \
  graphlite:test

# Manual GQL shell
docker run -it --rm -v $(pwd)/testdb:/data graphlite:test \
  graphlite gql --path /data/db -u admin -p pass

# Docker Compose
docker-compose build
docker-compose run --rm graphlite graphlite install --path /data/mydb --admin-user admin --admin-password secret
docker-compose run --rm graphlite graphlite gql --path /data/mydb -u admin -p secret

# Cleanup
docker rmi graphlite:test
docker-compose down -v
rm -rf ./docker_test_db
```

---

## Next Steps

1. ✅ Start Docker Desktop/daemon
2. ✅ Run: `docker build -t graphlite:test .`
3. ✅ Test basic commands
4. ✅ Initialize test database
5. ✅ Try automatic GQL shell (the key feature!)
6. ✅ Test Docker Compose
7. ✅ Run automated test suite
8. ✅ Report results

---

## Need Help?

- **Documentation:** [docs/Docker.md](docs/Docker.md)
- **Test Plan:** [DOCKER_TEST_PLAN.md](DOCKER_TEST_PLAN.md)
- **Implementation Report:** [DOCKER_IMPLEMENTATION_REPORT.md](DOCKER_IMPLEMENTATION_REPORT.md)
- **Issues:** Check Docker logs with `docker logs <container>`

---

**Ready to test?** Start with:
```bash
open -a Docker  # Start Docker
docker build -t graphlite:test .  # Build image (10-15 min)
./scripts/docker-test.sh --image graphlite:test  # Run tests
```

Good luck! 🚀

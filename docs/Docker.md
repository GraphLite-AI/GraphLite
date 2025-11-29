# GraphLite Docker Guide

Complete guide for building, deploying, and managing GraphLite using Docker and Docker Compose with multi-architecture support.

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Multi-Architecture Support](#multi-architecture-support)
- [Building Images](#building-images)
- [Docker Compose](#docker-compose)
- [Production Deployment](#production-deployment)
- [Advanced Topics](#advanced-topics)
- [Troubleshooting](#troubleshooting)

---

## Overview

GraphLite provides Docker support with the following features:

- **Multi-stage builds** - Optimized image size (~150-200 MB runtime)
- **Multi-architecture** - Supports AMD64/x86_64 and ARM64/aarch64
- **Cross-compilation** - Build for any architecture from any platform
- **Security-focused** - Non-root user, minimal attack surface
- **Production-ready** - Health checks, resource limits, restart policies
- **Docker Compose** - Development and production configurations

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     Docker Build Process                     │
├─────────────────────────────────────────────────────────────┤
│  Stage 1: Builder (rust:slim-bookworm)                      │
│  - Install build tools and cross-compilation toolchain      │
│  - Compile GraphLite for target architecture               │
│  - Optimize binary for size and performance                │
├─────────────────────────────────────────────────────────────┤
│  Stage 2: Runtime (ubuntu:22.04)                            │
│  - Minimal runtime dependencies                             │
│  - Non-root user (graphlite)                                │
│  - Copy optimized binary from builder                       │
│  - Configure volumes and environment                        │
└─────────────────────────────────────────────────────────────┘
```

---

## Prerequisites

### Required

- **Docker** 20.10 or later
- **Docker Compose** 2.0 or later (for compose files)
- **Docker Buildx** (included in Docker Desktop, required for multi-arch)

### Optional

- Docker Hub or container registry account (for pushing images)
- 4GB+ RAM for building (8GB+ recommended for multi-arch)
- 10GB+ free disk space

### Installation

<details>
<summary><b>macOS</b></summary>

```bash
# Install Docker Desktop (includes Buildx)
brew install --cask docker

# Start Docker Desktop and verify
docker --version
docker buildx version
docker-compose --version
```
</details>

<details>
<summary><b>Linux (Ubuntu/Debian)</b></summary>

```bash
# Install Docker
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

# Install Docker Compose
sudo apt-get update
sudo apt-get install docker-compose-plugin

# Add user to docker group
sudo usermod -aG docker $USER
newgrp docker

# Verify installation
docker --version
docker buildx version
docker compose version
```
</details>

<details>
<summary><b>Windows</b></summary>

1. Install [Docker Desktop for Windows](https://docs.docker.com/desktop/install/windows-install/)
2. Enable WSL 2 backend (recommended)
3. Verify installation:
   ```powershell
   docker --version
   docker buildx version
   docker compose version
   ```
</details>

---

## Quick Start

### 1. Build the Image

```bash
# Build for your current architecture
docker build -t graphlite:latest .

# Or use the build script
./scripts/docker-build.sh --native --load
```

### 2. Test the Image

```bash
# Check version
docker run --rm graphlite:latest graphlite --version

# Run test suite
./scripts/docker-test.sh
```

### 3. Initialize a Database

```bash
# Create database directory
mkdir -p ./mydb

# Initialize database
docker run -it --rm \
  -v $(pwd)/mydb:/data \
  graphlite:latest \
  graphlite install \
    --path /data/mydb \
    --admin-user admin \
    --admin-password secret
```

### 4. Start Interactive GQL Shell

The container automatically starts the GQL shell when you provide database credentials via environment variables:

```bash
# Method 1: Automatic GQL shell (recommended)
docker run -it --rm \
  -v $(pwd)/mydb:/data \
  -e GRAPHLITE_DB_PATH=/data/mydb \
  -e GRAPHLITE_USER=admin \
  -e GRAPHLITE_PASSWORD=secret \
  graphlite:latest

# Method 2: Explicit command
docker run -it --rm \
  -v $(pwd)/mydb:/data \
  graphlite:latest \
  graphlite gql --path /data/mydb -u admin -p secret
```

You should now see the `gql>` prompt where you can enter queries!

### 5. Using Docker Compose

```bash
# Initialize database (first time)
docker-compose run --rm graphlite graphlite install \
  --path /data/mydb --admin-user admin --admin-password secret

# Start GQL shell interactively
docker-compose run --rm graphlite graphlite gql \
  --path /data/mydb -u admin -p secret

# Or configure environment variables in docker-compose.yml and run:
# (Uncomment GRAPHLITE_DB_PATH, GRAPHLITE_USER, GRAPHLITE_PASSWORD in the file)
docker-compose run --rm graphlite

# Execute other commands
docker-compose run --rm graphlite graphlite --version

# Stop services
docker-compose down
```

---

## Multi-Architecture Support

GraphLite supports building and running on multiple architectures:

- **linux/amd64** (x86_64) - Intel/AMD 64-bit
- **linux/arm64** (aarch64) - ARM 64-bit (Apple Silicon, AWS Graviton, etc.)

### Why Multi-Architecture?

- **Flexibility** - Run on Intel, AMD, or ARM processors
- **Cloud Optimization** - Use ARM instances (AWS Graviton, Azure Ampere) for cost savings
- **Apple Silicon** - Native performance on M1/M2/M3 Macs
- **IoT/Edge** - Deploy on ARM-based edge devices

### Cross-Compilation

The Dockerfile uses cross-compilation to build for any target architecture from any build platform:

```bash
# Build on ARM Mac for AMD64 servers
docker buildx build --platform linux/amd64 -t graphlite:amd64 .

# Build on x86_64 for ARM servers
docker buildx build --platform linux/arm64 -t graphlite:arm64 .

# Build both architectures simultaneously
docker buildx build --platform linux/amd64,linux/arm64 -t graphlite:latest .
```

---

## Building Images

### Basic Build

```bash
# Build for current architecture
docker build -t graphlite:latest .

# Build with custom tag
docker build -t graphlite:v0.0.1 .

# Build with no cache (clean build)
docker build --no-cache -t graphlite:latest .
```

### Multi-Architecture Build

```bash
# Setup buildx builder (one-time setup)
docker buildx create --name graphlite-builder --driver docker-container --bootstrap --use

# Build for both architectures
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t graphlite:latest \
  .

# Build and push to registry
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t docker.io/username/graphlite:latest \
  --push \
  .

# Build and load to local Docker (single platform only)
docker buildx build \
  --platform linux/amd64 \
  -t graphlite:latest \
  --load \
  .
```

### Using the Build Script

The provided build script simplifies multi-architecture builds:

```bash
# Show help
./scripts/docker-build.sh --help

# Build for current architecture only
./scripts/docker-build.sh --native

# Build for AMD64/x86_64
./scripts/docker-build.sh --amd64

# Build for ARM64/aarch64
./scripts/docker-build.sh --arm64

# Build multi-arch images
./scripts/docker-build.sh

# Build with custom tag
./scripts/docker-build.sh --tag v0.0.1

# Build and push to registry
./scripts/docker-build.sh \
  --registry docker.io/username \
  --tag latest \
  --push

# Build without cache
./scripts/docker-build.sh --no-cache

# Build and load to local Docker
./scripts/docker-build.sh --native --load
```

### Build Arguments

Customize the build with build arguments:

```bash
docker build \
  --build-arg RUST_VERSION=1.83 \
  -t graphlite:latest \
  .
```

---

## Docker Compose

GraphLite includes two Docker Compose configurations:

### Development Configuration

**File:** [docker-compose.yml](../docker-compose.yml)

Features:
- Interactive mode (stdin/tty enabled)
- Volume mounts for persistent data
- Development-friendly settings
- Lower resource limits

```bash
# Initialize database (first time)
docker-compose run --rm graphlite graphlite install \
  --path /data/mydb --admin-user admin --admin-password secret

# Start interactive GQL shell
docker-compose run --rm graphlite graphlite gql \
  --path /data/mydb -u admin -p secret

# Or configure environment variables in docker-compose.yml:
# Uncomment GRAPHLITE_DB_PATH, GRAPHLITE_USER, GRAPHLITE_PASSWORD
# Then run:
docker-compose run --rm graphlite

# View logs (if running in background)
docker-compose logs -f

# Execute other commands
docker-compose run --rm graphlite graphlite --version
docker-compose exec graphlite bash

# Stop services
docker-compose down

# Stop and remove volumes (WARNING: deletes data)
docker-compose down -v
```

### Production Configuration

**File:** [docker-compose.prod.yml](../docker-compose.prod.yml)

Features:
- Optimized for production
- Higher resource limits
- Health checks
- Security hardening
- Restart policies
- Monitoring ready

```bash
# Set password securely
export GRAPHLITE_PASSWORD="your-secure-password"

# Initialize database (first time)
docker-compose -f docker-compose.prod.yml run --rm graphlite \
  graphlite install --path /data/production_db --admin-user admin --admin-password "$GRAPHLITE_PASSWORD"

# Deploy production stack
docker-compose -f docker-compose.prod.yml up -d

# Start interactive GQL shell
docker-compose -f docker-compose.prod.yml run --rm graphlite

# Or attach to running container
docker attach graphlite-prod

# Health check
docker-compose -f docker-compose.prod.yml ps

# View logs
docker-compose -f docker-compose.prod.yml logs -f graphlite

# Scale services (if load-balanced)
docker-compose -f docker-compose.prod.yml up -d --scale graphlite=3

# Stop services
docker-compose -f docker-compose.prod.yml down
```

---

## Production Deployment

### Pre-Deployment Checklist

- [ ] Build and test images locally
- [ ] Run security scans
- [ ] Configure backups
- [ ] Set up monitoring
- [ ] Configure resource limits
- [ ] Review security settings
- [ ] Document rollback procedure

### Build for Production

```bash
# Build optimized production images
./scripts/docker-build.sh \
  --tag v0.0.1 \
  --registry your-registry.io/graphlite \
  --push

# Or with Docker directly
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t your-registry.io/graphlite:v0.0.1 \
  --push \
  .
```

### Deploy with Docker Compose

```bash
# Set password environment variable
export GRAPHLITE_PASSWORD="your-secure-password"

# Initialize database (first time)
docker-compose -f docker-compose.prod.yml run --rm graphlite \
  graphlite install \
    --path /data/production_db \
    --admin-user admin \
    --admin-password "$GRAPHLITE_PASSWORD"

# Deploy
docker-compose -f docker-compose.prod.yml up -d

# Start GQL shell
docker-compose -f docker-compose.prod.yml run --rm graphlite

# Monitor
docker-compose -f docker-compose.prod.yml logs -f
```

### Deploy with Docker Swarm

```bash
# Initialize Swarm (if not already)
docker swarm init

# Deploy stack
docker stack deploy -c docker-compose.prod.yml graphlite-stack

# List services
docker service ls

# Scale service
docker service scale graphlite-stack_graphlite=3

# View logs
docker service logs -f graphlite-stack_graphlite

# Update service
docker service update --image graphlite:v0.0.2 graphlite-stack_graphlite

# Remove stack
docker stack rm graphlite-stack
```

### Backup and Restore

**Backup:**
```bash
# Backup data volume
docker run --rm \
  -v graphlite-prod-data:/data \
  -v $(pwd)/backups:/backup \
  ubuntu \
  tar czf /backup/graphlite-backup-$(date +%Y%m%d-%H%M%S).tar.gz -C /data .

# Automated daily backups (add to crontab)
0 2 * * * docker run --rm -v graphlite-prod-data:/data -v /backups:/backup ubuntu tar czf /backup/graphlite-backup-$(date +\%Y\%m\%d).tar.gz -C /data .
```

**Restore:**
```bash
# Restore from backup
docker run --rm \
  -v graphlite-prod-data:/data \
  -v $(pwd)/backups:/backup \
  ubuntu \
  tar xzf /backup/graphlite-backup-YYYYMMDD-HHMMSS.tar.gz -C /data
```

### Monitoring

**Health Checks:**
```bash
# Check container health
docker inspect --format='{{.State.Health.Status}}' graphlite-prod

# View health logs
docker inspect --format='{{json .State.Health}}' graphlite-prod | jq
```

**Resource Usage:**
```bash
# Monitor resources
docker stats graphlite-prod

# View container logs
docker logs -f graphlite-prod
```

**Metrics Collection:**
```yaml
# Add to docker-compose.prod.yml
services:
  prometheus:
    image: prom/prometheus
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    ports:
      - "9090:9090"
```

---

## Advanced Topics

### Custom Base Images

Modify the Dockerfile to use alternative base images:

```dockerfile
# Use Alpine for smaller image size
FROM alpine:3.19 AS runtime

# Use specific Ubuntu version
FROM ubuntu:24.04 AS runtime

# Use distroless for security
FROM gcr.io/distroless/cc-debian12 AS runtime
```

### Build Optimization

**Reduce image size:**
```bash
# Use multi-stage builds (already implemented)
# Strip debug symbols
RUN strip /build/graphlite

# Use UPX compression (optional, increases startup time)
RUN upx --best --lzma /build/graphlite
```

**Faster builds:**
```bash
# Use cache mounts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release

# Build dependencies separately
RUN cargo build --release --workspace --no-default-features
```

### Security Hardening

**Run as non-root (already implemented):**
```dockerfile
USER graphlite
```

**Read-only root filesystem:**
```yaml
services:
  graphlite:
    read_only: true
    tmpfs:
      - /tmp
```

**Security scanning:**
```bash
# Scan with Trivy
trivy image graphlite:latest

# Scan with Snyk
snyk container test graphlite:latest

# Scan with Docker Scout
docker scout cves graphlite:latest
```

### Network Configuration

**Custom networks:**
```yaml
networks:
  graphlite-network:
    driver: bridge
    ipam:
      config:
        - subnet: 172.28.0.0/16
          gateway: 172.28.0.1
```

**Port mapping (when HTTP API is available):**
```yaml
services:
  graphlite:
    ports:
      - "8080:8080"      # HTTP API
      - "127.0.0.1:8080:8080"  # Bind to localhost only
```

### Volume Management

**Named volumes:**
```bash
# Create volume
docker volume create graphlite-data

# Inspect volume
docker volume inspect graphlite-data

# Backup volume
docker run --rm -v graphlite-data:/data -v $(pwd):/backup \
  ubuntu tar czf /backup/backup.tar.gz -C /data .

# Restore volume
docker run --rm -v graphlite-data:/data -v $(pwd):/backup \
  ubuntu tar xzf /backup/backup.tar.gz -C /data
```

**Bind mounts:**
```yaml
services:
  graphlite:
    volumes:
      - type: bind
        source: /mnt/storage/graphlite
        target: /data
```

---

## Troubleshooting

### Common Issues

**Issue: Build fails with "no space left on device"**
```bash
# Clean up Docker
docker system prune -a --volumes

# Increase Docker disk size (Docker Desktop)
# Settings → Resources → Disk image size
```

**Issue: Cross-compilation fails**
```bash
# Ensure buildx is set up
docker buildx ls

# Create builder
docker buildx create --name graphlite-builder --driver docker-container --bootstrap --use

# Verify platforms
docker buildx inspect --bootstrap
```

**Issue: Container exits immediately**
```bash
# Check logs
docker logs graphlite-container

# Run interactively
docker run -it --rm graphlite:latest bash

# Check entry point
docker inspect graphlite:latest | jq '.[0].Config.Cmd'
```

**Issue: Permission denied accessing /data**
```bash
# Fix volume permissions
docker run --rm -v graphlite-data:/data ubuntu chown -R 1000:1000 /data

# Or run as root (not recommended for production)
docker run --user root -it graphlite:latest bash
```

**Issue: Image too large**
```bash
# Check image size
docker images graphlite:latest

# Analyze layers
docker history graphlite:latest

# Use dive for detailed analysis
dive graphlite:latest
```

### Debug Commands

```bash
# Inspect image
docker inspect graphlite:latest

# Check image layers
docker history graphlite:latest

# Shell into running container
docker exec -it graphlite-container bash

# Run container with debug logging
docker run -e RUST_LOG=debug -it graphlite:latest

# Check buildx builder
docker buildx inspect graphlite-builder

# View build cache
docker buildx du

# Prune build cache
docker buildx prune
```

### Getting Help

- **Documentation**: [GraphLite GitHub](https://github.com/GraphLite-AI/GraphLite)
- **Issues**: [GitHub Issues](https://github.com/GraphLite-AI/GraphLite/issues)
- **Docker Docs**: [Docker Documentation](https://docs.docker.com/)
- **Buildx Docs**: [Docker Buildx](https://docs.docker.com/buildx/working-with-buildx/)

---

## Summary

GraphLite provides comprehensive Docker support with:

✅ Multi-architecture builds (AMD64, ARM64)
✅ Cross-compilation from any platform
✅ Development and production Docker Compose configs
✅ Security-focused design (non-root user)
✅ Optimized image size (~150-200 MB)
✅ Automated build and test scripts
✅ Production deployment examples
✅ Backup and monitoring strategies

**Next Steps:**
1. Build your first image: `./scripts/docker-build.sh --native`
2. Test the image: `./scripts/docker-test.sh`
3. Start with Docker Compose: `docker-compose up -d`
4. Review production deployment guide above
5. Set up monitoring and backups

For more information, see:
- [README.md](../README.md) - Main documentation
- [Quick Start.md](Quick%20Start.md) - Getting started guide
- [Getting Started With GQL.md](Getting%20Started%20With%20GQL.md) - Query language reference

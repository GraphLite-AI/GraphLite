# GraphLite Docker - Quick Start Cheat Sheet

## 🚀 Start Here (5 Steps)

### 1️⃣ Start Docker
```bash
open -a Docker  # macOS
# Wait for Docker icon in menu bar
```

### 2️⃣ Build Image
```bash
cd /Users/mac/code/github/graphlite-ai/GraphLite
docker build -t graphlite:test .
# ⏱️ Takes 10-15 minutes (first time)
```

### 3️⃣ Initialize Database
```bash
mkdir -p ./testdb
docker run -it --rm -v $(pwd)/testdb:/data graphlite:test \
  graphlite install --path /data/db --admin-user admin --admin-password pass
```

### 4️⃣ Start GQL Shell (Automatic) ⭐
```bash
docker run -it --rm -v $(pwd)/testdb:/data \
  -e GRAPHLITE_DB_PATH=/data/db \
  -e GRAPHLITE_USER=admin \
  -e GRAPHLITE_PASSWORD=pass \
  graphlite:test

# You'll see: gql> █
```

### 5️⃣ Try Some Queries
```gql
CREATE SCHEMA /test
CREATE GRAPH /test/mygraph
SESSION SET GRAPH /test/mygraph
INSERT (:Person {name: 'Alice', age: 30})
MATCH (p:Person) RETURN p.name, p.age
EXIT
```

---

## 📋 Essential Commands

### Build
```bash
# Native architecture
docker build -t graphlite:test .

# Or use script
./scripts/docker-build.sh --native --load
```

### Test
```bash
# Version check
docker run --rm graphlite:test graphlite --version

# Run test suite
./scripts/docker-test.sh --image graphlite:test
```

### GQL Shell
```bash
# Automatic (with env vars) - RECOMMENDED
docker run -it --rm -v $(pwd)/testdb:/data \
  -e GRAPHLITE_DB_PATH=/data/db \
  -e GRAPHLITE_USER=admin \
  -e GRAPHLITE_PASSWORD=pass \
  graphlite:test

# Manual (explicit command)
docker run -it --rm -v $(pwd)/testdb:/data graphlite:test \
  graphlite gql --path /data/db -u admin -p pass
```

### Docker Compose
```bash
# Build
docker-compose build

# Initialize
docker-compose run --rm graphlite graphlite install \
  --path /data/mydb --admin-user admin --admin-password secret

# GQL Shell
docker-compose run --rm graphlite graphlite gql \
  --path /data/mydb -u admin -p secret
```

---

## 🏗️ Multi-Architecture

```bash
# AMD64 (x86_64)
./scripts/docker-build.sh --amd64 --tag amd64 --load

# ARM64 (aarch64)
./scripts/docker-build.sh --arm64 --tag arm64

# Both
./scripts/docker-build.sh
```

---

## 🧹 Cleanup

```bash
# Remove image
docker rmi graphlite:test

# Clean system
docker system prune -a

# Remove test database
rm -rf ./testdb
```

---

## ❓ Troubleshooting

**Docker not running?**
```bash
open -a Docker  # macOS
```

**Build fails?**
```bash
docker system prune -a  # Clean and retry
```

**Permission issues?**
```bash
chmod -R 777 ./testdb
```

---

## 📚 Full Docs

- **Complete Guide:** [docs/Docker.md](docs/Docker.md)
- **Manual Testing:** [MANUAL_TEST_GUIDE.md](MANUAL_TEST_GUIDE.md)
- **Test Plan:** [DOCKER_TEST_PLAN.md](DOCKER_TEST_PLAN.md)

---

## ✅ Success Checklist

- [ ] Docker started
- [ ] Image builds successfully
- [ ] Version command works
- [ ] Database initializes
- [ ] **GQL shell starts automatically** ⭐
- [ ] Queries execute
- [ ] Docker Compose works

---

**Ready?** Run this:
```bash
open -a Docker && sleep 30 && docker build -t graphlite:test .
```

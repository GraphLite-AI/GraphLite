# GraphLite JavaScript Bindings - Installation Guide

Complete guide for installing and setting up the GraphLite JavaScript bindings.

## Prerequisites

### System Requirements
- **Node.js**: Version 14.0.0 or higher
- **npm**: Version 6.0.0 or higher (comes with Node.js)
- **Rust**: Latest stable version (for building FFI library)
- **Cargo**: Rust package manager (comes with Rust)

### Platform-Specific Requirements

#### macOS
```bash
# Install Xcode Command Line Tools (if not already installed)
xcode-select --install
```

#### Linux (Ubuntu/Debian)
```bash
# Install build essentials
sudo apt-get update
sudo apt-get install build-essential
```

#### Windows
- Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/)
- Or install Visual Studio Community with C++ development tools

## Installation Steps

### Step 1: Install Rust (if not already installed)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Verify installation:
```bash
rustc --version
cargo --version
```

### Step 2: Build the GraphLite FFI Library

Navigate to the GraphLite root directory:

```bash
cd /path/to/GraphLite
cargo build --release -p graphlite-ffi
```

This creates the shared library at:
- **macOS**: `target/release/libgraphlite_ffi.dylib`
- **Linux**: `target/release/libgraphlite_ffi.so`
- **Windows**: `target/release/graphlite_ffi.dll`

### Step 3: Install JavaScript Bindings

#### Option A: Local Development

```bash
cd bindings/javascript
npm install
```

#### Option B: Install from npm (when published)

```bash
npm install graphlite
```

## Verification

### Test the Installation

Create a test file `test.js`:

```javascript
const { GraphLite } = require('graphlite');

console.log('GraphLite version:', GraphLite.version());

const db = new GraphLite('./test_db');
const session = db.createSession('test');
console.log('Session created:', session);
db.close();
console.log('Success!');
```

Run it:
```bash
node test.js
```

### Run Examples

```bash
node examples/basic-usage.js
```

### Run Tests

```bash
npm test
```

## Troubleshooting

### Error: "Could not find GraphLite FFI library"

**Solution 1**: Build the FFI library
```bash
cd /path/to/GraphLite
cargo build --release -p graphlite-ffi
```

**Solution 2**: Set environment variable
```bash
export GRAPHLITE_FFI_LIB=/path/to/GraphLite/target/release/libgraphlite_ffi.dylib
```

Add to your shell profile for persistence:
```bash
echo 'export GRAPHLITE_FFI_LIB=/path/to/GraphLite/target/release/libgraphlite_ffi.dylib' >> ~/.zshrc
source ~/.zshrc
```

### Error: "ffi-napi installation failed"

**On macOS:**
```bash
xcode-select --install
npm install --build-from-source
```

**On Linux:**
```bash
sudo apt-get install build-essential
npm install --build-from-source
```

**On Windows:**
- Ensure Visual Studio Build Tools are installed
- Run npm in a Visual Studio Developer Command Prompt

### Error: "Cannot find module 'ffi-napi'"

```bash
cd bindings/javascript
npm install ffi-napi ref-napi ref-struct-di
```

### Error: Module loading issues

Clear npm cache and reinstall:
```bash
rm -rf node_modules package-lock.json
npm cache clean --force
npm install
```

### Permission Denied Errors

**On macOS/Linux:**
```bash
sudo chown -R $(whoami) ~/.npm
sudo chown -R $(whoami) /usr/local/lib/node_modules
```

### Library Path Issues

If the FFI library is in a non-standard location, create a symlink:

**macOS:**
```bash
sudo ln -s /path/to/GraphLite/target/release/libgraphlite_ffi.dylib /usr/local/lib/
```

**Linux:**
```bash
sudo ln -s /path/to/GraphLite/target/release/libgraphlite_ffi.so /usr/local/lib/
sudo ldconfig
```

## Environment Variables

### GRAPHLITE_FFI_LIB
Path to the FFI library (optional):
```bash
export GRAPHLITE_FFI_LIB=/custom/path/to/libgraphlite_ffi.dylib
```

### NODE_ENV
Set to 'development' or 'production':
```bash
export NODE_ENV=production
```

## Development Setup

For development work on the bindings:

```bash
cd bindings/javascript

# Install all dependencies including dev dependencies
npm install

# Run tests in watch mode
npm run test:watch

# Run linter
npm run lint

# Format code
npm run format
```

## IDE Setup

### Visual Studio Code

Install recommended extensions:
- ESLint
- Prettier
- Jest

Create `.vscode/settings.json`:
```json
{
  "editor.formatOnSave": true,
  "editor.defaultFormatter": "esbenp.prettier-vscode",
  "eslint.enable": true
}
```

### WebStorm/IntelliJ

1. Go to Preferences → Languages & Frameworks → JavaScript
2. Set JavaScript language version to ECMAScript 2021
3. Enable ESLint: Preferences → Languages & Frameworks → JavaScript → Code Quality Tools → ESLint
4. Enable Prettier: Preferences → Languages & Frameworks → JavaScript → Prettier

## Docker Setup (Optional)

Create a `Dockerfile` for containerized development:

```dockerfile
FROM node:18-alpine

RUN apk add --no-cache \
    build-base \
    python3 \
    rust \
    cargo

WORKDIR /app

COPY package*.json ./
RUN npm install

COPY . .

CMD ["npm", "test"]
```

Build and run:
```bash
docker build -t graphlite-js .
docker run graphlite-js
```

## CI/CD Integration

### GitHub Actions

Example workflow (`.github/workflows/test.yml`):

```yaml
name: Test JavaScript Bindings

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v2
    
    - name: Setup Node.js
      uses: actions/setup-node@v2
      with:
        node-version: '18'
    
    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
    
    - name: Build FFI
      run: cargo build --release -p graphlite-ffi
    
    - name: Install dependencies
      working-directory: bindings/javascript
      run: npm install
    
    - name: Run tests
      working-directory: bindings/javascript
      run: npm test
```

## Next Steps

- Read the [README.md](README.md) for API documentation
- Explore [examples/](examples/) for usage patterns
- Check out [test/](test/) for more examples
- Join the community discussions

## Support

- GitHub Issues: https://github.com/deepgraph/graphlite/issues
- Documentation: https://github.com/deepgraph/graphlite/tree/main/bindings/javascript

## License

Apache-2.0

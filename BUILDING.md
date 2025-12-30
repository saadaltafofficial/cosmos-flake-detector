# 🔨 Building cosmos-flake-detector

Complete build instructions for all platforms.

## System Requirements

- **Rust**: 1.70 or newer
- **Cargo**: Included with Rust
- **OS**: Linux, macOS, Windows, or WSL2
- **Memory**: 2GB RAM minimum for compilation
- **Disk**: ~500MB for dependencies and build artifacts

## Installing Rust

If you don't have Rust installed:

### Linux / macOS / WSL2
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Windows
Download and run: https://rustup.rs/

Verify installation:
```bash
rustc --version  # Should show 1.70+
cargo --version
```

## Building from Source

### 1. Clone Repository
```bash
git clone https://github.com/yourusername/cosmos-flake-detector
cd cosmos-flake-detector
```

### 2. Build (Development)
```bash
cargo build
```
Binary location: `./target/debug/flake-detector`

### 3. Build (Production/Optimized)
```bash
cargo build --release
```
Binary location: `./target/release/flake-detector`

**Build time**: 2-5 minutes depending on your system.

## Build Profiles

### Debug Build (Development)
```bash
cargo build
```
- **Pros**: Fast compilation, includes debug symbols
- **Cons**: Slower runtime, larger binary (~15MB)
- **Use for**: Development, debugging

### Release Build (Production)
```bash
cargo build --release
```
- **Pros**: Optimized for speed, smaller binary (~5MB)
- **Cons**: Slower compilation
- **Use for**: Production, benchmarking

### Custom Optimization
Edit `Cargo.toml`:
```toml
[profile.release]
opt-level = 3        # Maximum optimization
lto = true          # Link-time optimization
codegen-units = 1   # Better optimization
strip = true        # Strip debug symbols
```

## Platform-Specific Instructions

### Linux

#### Ubuntu / Debian
```bash
# Install build essentials
sudo apt update
sudo apt install build-essential pkg-config libssl-dev

# Build
cargo build --release
```

#### Fedora / RHEL
```bash
sudo dnf install gcc openssl-devel

cargo build --release
```

#### Arch Linux
```bash
sudo pacman -S base-devel openssl

cargo build --release
```

### macOS

```bash
# Xcode command line tools (if not installed)
xcode-select --install

# Build
cargo build --release
```

**Apple Silicon (M1/M2/M3)**:
```bash
# Native ARM build (recommended)
cargo build --release

# x86_64 build (for compatibility)
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin
```

### Windows

#### Using Visual Studio Build Tools
```powershell
# Install Visual Studio Build Tools 2019 or newer
# Download from: https://visualstudio.microsoft.com/downloads/

# Build
cargo build --release
```

#### Using WSL2 (Recommended)
```bash
# Inside WSL2 Ubuntu
sudo apt update
sudo apt install build-essential pkg-config libssl-dev

cargo build --release
```

### Docker Build

```dockerfile
# Dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/flake-detector /usr/local/bin/
CMD ["flake-detector"]
```

Build:
```bash
docker build -t flake-detector .
docker run flake-detector --help
```

## Cross-Compilation

### Linux → Windows
```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

### Linux → macOS
```bash
# Requires osxcross toolchain
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin
```

### macOS → Linux
```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

## Testing the Build

### Run Tests
```bash
cargo test
```

### Check Code Quality
```bash
# Lint with Clippy
cargo clippy

# Format check
cargo fmt --check

# Security audit
cargo audit
```

### Verify Binary
```bash
# Check it runs
./target/release/flake-detector --help

# Test against live endpoint
./target/release/flake-detector \
  --endpoints https://zigchain-mainnet-rpc-sanatry-01.wickhub.cc \
  --duration 30
```

## Troubleshooting

### Error: "failed to compile openssl-sys"

**Linux**:
```bash
sudo apt install pkg-config libssl-dev
```

**macOS**:
```bash
brew install openssl
export OPENSSL_DIR=$(brew --prefix openssl)
```

**Windows**: Install Visual Studio Build Tools

### Error: "linker 'cc' not found"

**Linux**:
```bash
sudo apt install build-essential
```

**macOS**:
```bash
xcode-select --install
```

### Error: "failed to download dependencies"

```bash
# Try with vendored OpenSSL
cargo build --release --features vendored
```

### Slow Compilation

```bash
# Use more CPU cores (if you have 8 cores)
cargo build --release -j 8

# Use mold linker (Linux)
sudo apt install mold
RUSTFLAGS="-C link-arg=-fuse-ld=mold" cargo build --release
```

### Out of Memory

```bash
# Reduce parallelism
cargo build --release -j 2
```

### Cannot find binary after build

```bash
# Debug build
ls -la target/debug/flake-detector

# Release build  
ls -la target/release/flake-detector

# If extension is wrong, it might be renamed:
mv target/release/flake-detector.exe target/release/flake-detector
```

## Clean Build

```bash
# Remove all build artifacts
cargo clean

# Fresh build
cargo build --release
```

## Build Size Optimization

### Minimal Binary Size
Add to `Cargo.toml`:
```toml
[profile.release]
opt-level = "z"     # Optimize for size
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### Using UPX Compression
```bash
# Install UPX
sudo apt install upx  # Linux
brew install upx      # macOS

# Compress binary
upx --best --lzma target/release/flake-detector
```

Can reduce binary from ~5MB to ~2MB.

## Installation

### System-Wide Installation (Linux/macOS)
```bash
cargo build --release
sudo cp target/release/flake-detector /usr/local/bin/
```

### User Installation
```bash
cargo install --path .
```
Binary will be in `~/.cargo/bin/`

### From crates.io (once published)
```bash
cargo install flake-detector
```

## Build Artifacts

After successful build:
```
target/
├── debug/               # Debug builds
│   └── flake-detector
├── release/             # Release builds
│   └── flake-detector   # Main binary (~5MB)
└── CACHEDIR.TAG
```

## Build Time Benchmarks

Typical build times (fresh build with all dependencies):

| System | Debug | Release |
|--------|-------|---------|
| M1 Mac | 90s | 180s |
| Intel i7 (8 cores) | 120s | 240s |
| GitHub Actions | 150s | 300s |
| Raspberry Pi 4 | 600s | 1200s |

Rebuild (no dependency changes): 5-15 seconds

## CI/CD Integration

See `.github/workflows/ci.yml` for automated builds in:
- GitHub Actions
- GitLab CI
- Jenkins
- Circle CI

---

**Need help?** Open an issue with:
- OS and version
- Rust version (`rustc --version`)
- Full error message
- Build command used
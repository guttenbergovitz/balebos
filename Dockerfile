# ==========================================
# Stage 1: Builder
# ==========================================
FROM --platform=$BUILDPLATFORM rust:1.88-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    wget \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# Detect architecture and download correct DuckDB library
ARG TARGETARCH
ENV DUCKDB_VERSION=v1.1.1
RUN case "${TARGETARCH}" in \
    "amd64") DUCK_ARCH="amd64" ;; \
    "arm64") DUCK_ARCH="aarch64" ;; \
    *) DUCK_ARCH="amd64" ;; \
    esac && \
    wget -q https://github.com/duckdb/duckdb/releases/download/${DUCKDB_VERSION}/libduckdb-linux-${DUCK_ARCH}.zip && \
    unzip -o libduckdb-linux-${DUCK_ARCH}.zip -d /opt/duckdb && \
    rm libduckdb-linux-${DUCK_ARCH}.zip

# Set environment variables for the Rust linker
ENV DUCKDB_LIB_DIR=/opt/duckdb
ENV LIBRARY_PATH=/opt/duckdb

# Copy project files
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY data ./data

# Build the application in release mode
RUN cargo build --release

# ==========================================
# Stage 2: Runtime
# ==========================================
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    whois \
    ca-certificates \
    wget \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# Install ProjectDiscovery tools with architecture detection
ARG TARGETARCH
ENV PD_VERSION_SUBFINDER=2.6.6
ENV PD_VERSION_HTTPX=1.6.8
ENV PD_VERSION_DNSX=1.2.1

RUN case "${TARGETARCH}" in \
    "amd64") ARCH="amd64" ;; \
    "arm64") ARCH="arm64" ;; \
    *) ARCH="amd64" ;; \
    esac && \
    wget -q https://github.com/projectdiscovery/subfinder/releases/download/v${PD_VERSION_SUBFINDER}/subfinder_${PD_VERSION_SUBFINDER}_linux_${ARCH}.zip && \
    unzip -o subfinder_${PD_VERSION_SUBFINDER}_linux_${ARCH}.zip -d /usr/local/bin/ && \
    rm subfinder_${PD_VERSION_SUBFINDER}_linux_${ARCH}.zip && \
    \
    wget -q https://github.com/projectdiscovery/httpx/releases/download/v${PD_VERSION_HTTPX}/httpx_${PD_VERSION_HTTPX}_linux_${ARCH}.zip && \
    unzip -o httpx_${PD_VERSION_HTTPX}_linux_${ARCH}.zip -d /usr/local/bin/ && \
    rm httpx_${PD_VERSION_HTTPX}_linux_${ARCH}.zip && \
    \
    wget -q https://github.com/projectdiscovery/dnsx/releases/download/v${PD_VERSION_DNSX}/dnsx_${PD_VERSION_DNSX}_linux_${ARCH}.zip && \
    unzip -o dnsx_${PD_VERSION_DNSX}_linux_${ARCH}.zip -d /usr/local/bin/ && \
    rm dnsx_${PD_VERSION_DNSX}_linux_${ARCH}.zip

# Copy DuckDB shared library from builder
COPY --from=builder /opt/duckdb/libduckdb.so /usr/lib/

# Copy the compiled Rust binary
COPY --from=builder /app/target/release/balebos /usr/local/bin/

# Create data directory
RUN mkdir -p /app/data

# Setup environment
ENV BALEBOS_STORAGE=duckdb
ENV TERM=xterm-256color

ENTRYPOINT ["balebos"]
CMD ["ui"]

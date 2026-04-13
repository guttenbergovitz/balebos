# ==========================================
# Stage 1: Builder
# ==========================================
FROM --platform=$BUILDPLATFORM rust:1.88-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy project files
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY data ./data

# Build the application in release mode
RUN cargo build --release

# ==========================================
# Stage 2: Scanner binaries
# ==========================================
FROM --platform=$BUILDPLATFORM debian:bookworm-slim AS scanner-base

RUN apt-get update && apt-get install -y \
    wget \
    unzip \
    && rm -rf /var/lib/apt/lists/*

ARG TARGETARCH
ENV PD_VERSION_SUBFINDER=2.6.6
ENV PD_VERSION_HTTPX=1.6.8
ENV PD_VERSION_DNSX=1.2.1
ENV PD_VERSION_ASNMAP=1.1.0

RUN case "${TARGETARCH}" in \
    "amd64") ARCH="amd64" ;; \
    "arm64") ARCH="arm64" ;; \
    *) ARCH="amd64" ;; \
    esac && \
    mkdir -p /tools && \
    printf '%s' "${ARCH}" > /tools/.arch

# ------------------------------------------
FROM scanner-base AS subfinder
RUN ARCH=$(cat /tools/.arch) && \
    wget -q "https://github.com/projectdiscovery/subfinder/releases/download/v${PD_VERSION_SUBFINDER}/subfinder_${PD_VERSION_SUBFINDER}_linux_${ARCH}.zip" -O /tmp/subfinder.zip && \
    unzip -o /tmp/subfinder.zip -d /tools/ && \
    rm /tmp/subfinder.zip

# ------------------------------------------
FROM scanner-base AS httpx
RUN ARCH=$(cat /tools/.arch) && \
    wget -q "https://github.com/projectdiscovery/httpx/releases/download/v${PD_VERSION_HTTPX}/httpx_${PD_VERSION_HTTPX}_linux_${ARCH}.zip" -O /tmp/httpx.zip && \
    unzip -o /tmp/httpx.zip -d /tools/ && \
    rm /tmp/httpx.zip

# ------------------------------------------
FROM scanner-base AS dnsx
RUN ARCH=$(cat /tools/.arch) && \
    wget -q "https://github.com/projectdiscovery/dnsx/releases/download/v${PD_VERSION_DNSX}/dnsx_${PD_VERSION_DNSX}_linux_${ARCH}.zip" -O /tmp/dnsx.zip && \
    unzip -o /tmp/dnsx.zip -d /tools/ && \
    rm /tmp/dnsx.zip

# ------------------------------------------
FROM scanner-base AS asnmap
RUN ARCH=$(cat /tools/.arch) && \
    wget -q "https://github.com/projectdiscovery/asnmap/releases/download/v${PD_VERSION_ASNMAP}/asnmap_${PD_VERSION_ASNMAP}_linux_${ARCH}.zip" -O /tmp/asnmap.zip && \
    unzip -o /tmp/asnmap.zip -d /tools/ && \
    rm /tmp/asnmap.zip

# ==========================================
# Stage 3: Runtime
# ==========================================
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    whois \
    ca-certificates \
    wget \
    unzip \
    dnsutils \
    && rm -rf /var/lib/apt/lists/*

# Copy scanner binaries from individual stages
COPY --from=subfinder /tools/subfinder /usr/local/bin/
COPY --from=httpx /tools/httpx /usr/local/bin/
COPY --from=dnsx /tools/dnsx /usr/local/bin/
COPY --from=asnmap /tools/asnmap /usr/local/bin/

# Copy the compiled Rust binary
COPY --from=builder /app/target/release/balebos /usr/local/bin/

# Create data directory
RUN mkdir -p /app/data

# BALEBOS_STORAGE is kept to suppress probe stdout spam in TUI mode
ENV BALEBOS_STORAGE=hybrid
ENV TERM=xterm-256color

ENTRYPOINT ["balebos"]
CMD ["ui"]

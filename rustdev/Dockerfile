# Dockerfile for building Rust firmware for the MAX78000 Feather board
# Adapted from the original C++ build process

FROM ubuntu:24.04

LABEL version="0.1"
LABEL description="Rust-based firmware build environment for MAX78000"

ARG DEBIAN_FRONTEND=noninteractive
WORKDIR /root

# Install requisite packages
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    gcc-arm-none-eabi \
    binutils-arm-none-eabi \
    git \
    curl \
    ca-certificates \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Install Rust nightly and cargo tools
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    && . "$HOME/.cargo/env" \
    && rustup default nightly \
    && rustup target add thumbv7em-none-eabihf \
    && cargo install cargo-generate

WORKDIR /decoder

# Set environment variables for Rust
ENV PATH="/root/.cargo/bin:$PATH"
ENV CARGO_TARGET_DIR=/decoder/target

# Define build process
ENTRYPOINT ["bash", "-c", "cargo build && cp target/thumbv7em-none-eabihf/debug/eCTF_2025_MSU /out/max78000.elf && arm-none-eabi-objcopy -O binary /out/max78000.elf /out/max78000.bin"]

# Build:
# docker build -t decoder ./decoder

# Sample run command:
# docker run -v ./rustdev/:/decoder -v ./global.secrets:/global.secrets:ro -v ./deadbeef_build:/out -e DECODER_ID=0xdeadbeef decoder


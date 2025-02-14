# Dockerfile for building Rust firmware for the MAX78000 Feather board
# Adapted from the original C++ build process

FROM ubuntu:24.04

LABEL version="0.1"
LABEL description="Rust-based firmware build environment for MAX78000"

ARG DEBIAN_FRONTEND=noninteractive
WORKDIR /root

# Install requisite packages
RUN apt-get update && apt-get upgrade -y && apt-get install -y \
    gdb \
    gdb-multiarch \
    gcc-arm-none-eabi \
    binutils-arm-none-eabi \
    python3.10 \
    git \
    wget \
    unzip \
    curl \
    clang \
    lld \
    pkg-config \
    libssl-dev

# Install Rust nightly and cargo tools
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    && . "$HOME/.cargo/env" \
    && rustup default nightly \
    && rustup target add thumbv7em-none-eabihf \
    && cargo install cargo-generate

# Install MSDK
RUN wget https://github.com/analogdevicesinc/msdk/archive/refs/tags/v2024_02.zip \
    && unzip v2024_02.zip \
    && rm -f v2024_02.zip

ENV MAXIM_PATH=/root/msdk-2024_02

WORKDIR /firmware

# Set environment variables for Rust
ENV PATH="/root/.cargo/bin:$PATH"
ENV CARGO_TARGET_DIR=/firmware/target

# Define build process
ENTRYPOINT ["bash", "-c", "mkdir -p ./build_out && cargo build --example ${EXAMPLE} && cp target/thumbv7em-none-eabihf/debug/examples/${EXAMPLE} ./build_out/${EXAMPLE}.elf && arm-none-eabi-objcopy -O binary ./build_out/${EXAMPLE}.elf ./build_out/${EXAMPLE}.bin"]

# Build:
# docker build -t rust-firmware-builder .

# Sample run command:
# docker run --rm -v ./build_out:/out -v ./:/firmware -e EXAMPLE=blinky rust-firmware-builder


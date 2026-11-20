FROM rust:1.90.0-bookworm@sha256:3f6e6f8d8725a65a2db964bb828850f888d430c68784d661f753144e5d787207

RUN dpkg --add-architecture i386 \
    && apt-get update -o Acquire::Retries=3 \
    && apt-get install -y --no-install-recommends \
        gcc-multilib g++-multilib pkg-config \
        libudev-dev libudev-dev:i386 \
        libwayland-dev libwayland-dev:i386 \
        libx11-dev libx11-dev:i386 \
        libxkbcommon-dev libxkbcommon-dev:i386 \
    && rustup component add clippy rustfmt \
    && rustup target add i686-unknown-linux-gnu \
    && rm -rf /var/lib/apt/lists/*

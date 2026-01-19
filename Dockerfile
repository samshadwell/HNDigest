# Stage 1: Build Rust binary with musl for static linking
FROM rust:1.92 AS builder

RUN rustup target add x86_64-unknown-linux-musl && \
    apt-get update && apt-get install -y musl-tools

WORKDIR /app

# Copy the Rust project
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY templates/ templates/

# Build for release with musl target (statically linked)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --target x86_64-unknown-linux-musl && \
    cp target/x86_64-unknown-linux-musl/release/hndigest bootstrap

# Stage 2: Deploy to Lambda
FROM amazon/aws-cli:2.33.2

RUN yum install -y zip

WORKDIR /var/task

COPY --from=builder /app/bootstrap .
RUN zip -9yr lambda.zip bootstrap

# Deployment command
CMD ["lambda", "update-function-code", "--function-name", "HNDigest", "--zip-file", "fileb://lambda.zip"]

# Stage 1: Build Rust binary with musl for static linking
FROM messense/rust-musl-cross:x86_64-musl AS builder
WORKDIR /home/rust/src

# Copy the Rust project
COPY rust/ .

# Build for release with musl target (statically linked)
RUN cargo build --release --target x86_64-unknown-linux-musl

# Rename binary to bootstrap
RUN cp target/x86_64-unknown-linux-musl/release/hndigest bootstrap

# Stage 2: Prepare deployment artifact
FROM amazonlinux:2023

RUN yum install -y zip unzip aws-cli

WORKDIR /var/task

COPY --from=builder /home/rust/src/bootstrap .

RUN zip -9yr lambda.zip bootstrap

# Deployment command
CMD ["sh", "-c", "aws lambda update-function-configuration --function-name ${LAMBDA_FUNCTION_NAME:-HNDigest} --runtime provided.al2023 --handler bootstrap && sleep 5 && aws lambda update-function-code --function-name ${LAMBDA_FUNCTION_NAME:-HNDigest} --zip-file fileb://lambda.zip"]

# Stage 1: Build Rust binary
FROM rust:1.77 as builder
WORKDIR /usr/src/app

# Copy the Rust project
COPY rust/ .

# Build for release
# We assume x86_64 target for AWS Lambda (or matching the user's setup)
RUN cargo build --release

# Rename binary to bootstrap
RUN cp target/release/hndigest bootstrap

# Stage 2: Prepare deployment artifact
FROM amazonlinux:2023

RUN yum install -y zip unzip aws-cli

WORKDIR /var/task

COPY --from=builder /usr/src/app/bootstrap .

RUN zip -9yr lambda.zip bootstrap

# Deployment command
CMD ["sh", "-c", "aws lambda update-function-configuration --function-name HNDigest --runtime provided.al2023 --handler bootstrap && aws lambda update-function-code --function-name HNDigest --zip-file fileb://lambda.zip"]

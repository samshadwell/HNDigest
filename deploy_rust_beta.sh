#!/bin/bash

# Configuration
FUNCTION_NAME="HNDigestRustBeta"

set -e

echo "Building Docker image..."
docker build -t hndigest-rust-build .

echo "Host AWS Access Key: $(aws configure get aws_access_key_id | sed 's/.\{16\}$/****/')"

# Get credentials
ACCESS_KEY="$(aws configure get aws_access_key_id)"
SECRET_KEY="$(aws configure get aws_secret_access_key)"
REGION="$(aws configure get region)"
REGION="${REGION:-us-west-2}" # Default to us-west-2 if not set

echo "Deploying to Lambda function: $FUNCTION_NAME in region $REGION"

docker run --rm \
  -e AWS_ACCESS_KEY_ID=$ACCESS_KEY \
  -e AWS_SECRET_ACCESS_KEY=$SECRET_KEY \
  -e AWS_DEFAULT_REGION=$REGION \
  -e LAMBDA_FUNCTION_NAME=$FUNCTION_NAME \
  hndigest-rust-build

echo "Deployment complete!"

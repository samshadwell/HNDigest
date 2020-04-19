#!/bin/bash

set -e

docker build -t hndigest-build .

ACCESS_KEY="$(aws configure get aws_access_key_id)"
SECRET_KEY="$(aws configure get aws_secret_access_key)"

docker run --rm -e AWS_ACCESS_KEY_ID=$ACCESS_KEY -e AWS_SECRET_ACCESS_KEY=$SECRET_KEY hndigest-build
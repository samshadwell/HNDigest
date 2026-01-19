# Infrastructure

This directory contains OpenTofu configurations for deploying HNDigest to AWS.

## Architecture

- **Lambda** (Rust, `provided.al2023`) - Runs the digest generation
- **DynamoDB** - Stores subscriber data and sent story tracking
- **EventBridge** - Triggers the Lambda on a daily schedule (5:00 AM UTC by default)
- **SES** - Sends digest emails
- **S3** - Stores OpenTofu state (remote backend) with native S3 locking

## Prerequisites

- [OpenTofu](https://opentofu.org/) >= 1.6.0
- [AWS CLI](https://aws.amazon.com/cli/) configured with credentials
- [Docker](https://www.docker.com/) (for building and deploying Lambda code)

## Setup

### 1. Create the State Bucket

The OpenTofu state is stored in S3. First, create the bucket using the bootstrap configuration:

```bash
# Assuming you are in /infrastructure
cd bootstrap
tofu init
tofu apply -var="bucket_name=YOUR_BUCKET_NAME"
cd ..
```

Replace `YOUR_BUCKET_NAME` with a globally unique bucket name. I've taken `hndigest-tfstate` so it can't be that!
Note that this will result in some local-only state. While the tfstate of the rest of this project is stored remotely,
this configuration describing the remote backend bucket itself will be local-only. This is fine as the bootstrap configuration
should never need to be modified.

If the bootstrap `.tfstate` is lost and you need to modify the bucket for any reason, you can import it:

```bash
tofu import aws_s3_bucket.tfstate YOUR_BUCKET_NAME
```

### 2. Configure the Backend

Update `infrastructure/versions.tf`, replacing `hndigest-tfstate` with your bucket name:

```hcl
# infrastructure/versions.tf
backend "s3" {
  bucket       = "YOUR_BUCKET_NAME"
  key          = "infrastructure/terraform.tfstate"
  region       = "us-west-2"
  use_lockfile = true
}
```

### 3. Set Variables

Copy `infrastructure/terraform.tfvars.example` to `infrastructure/terraform.tfvars` and update the values
as appropriate.

### 4. Deploy Infrastructure

```bash
# From /infrastructure
tofu init
tofu apply
```

### 5. Verify SES Domain

After `tofu apply`, you must verify your SES domain:

1. Go to the [AWS SES Console](https://console.aws.amazon.com/ses/)
2. Navigate to **Verified identities**
3. Find your domain and complete DNS verification by adding the required records

If your SES account is in sandbox mode, you'll also need to verify recipient email addresses or request production access.

### 6. Deploy Lambda Code

Infrastructure deployment creates a placeholder Lambda. Deploy the actual code:

```bash
cd ..  # back to project root
./deploy.sh
```

## Variables Reference

| Variable | Description | Default |
|----------|-------------|---------|
| `ses_from_email` | Email address to send digests from | (required) |
| `ses_reply_to_email` | Reply-to email address | (required) |
| `aws_region` | AWS region for all resources | `us-west-2` |
| `project_name` | Project name used for resource naming | `HNDigest` |
| `lambda_memory_size` | Lambda memory in MB | `256` |
| `lambda_timeout` | Lambda timeout in seconds | `60` |
| `schedule_expression` | EventBridge cron expression | `cron(0 5 * * ? *)` |

## Outputs

After deployment, OpenTofu outputs:

- `lambda_function_name` - Name of the Lambda function
- `lambda_function_arn` - ARN of the Lambda function
- `dynamodb_table_name` - Name of the DynamoDB table
- `dynamodb_table_arn` - ARN of the DynamoDB table
- `eventbridge_rule_arn` - ARN of the EventBridge schedule rule

## Directory Structure

```
infrastructure/
├── bootstrap/          # State bucket setup (run once)
│   ├── main.tf
│   ├── outputs.tf
│   └── versions.tf
├── main.tf             # AWS provider configuration
├── versions.tf         # OpenTofu and provider versions, backend config
├── variables.tf        # Input variables
├── outputs.tf          # Output values
├── locals.tf           # Local values
├── lambda.tf           # Lambda function
├── dynamodb.tf         # DynamoDB table
├── eventbridge.tf      # EventBridge schedule
├── iam.tf              # IAM roles and policies
└── ses.tf              # SES domain identity
```

## Destroying

To tear down the infrastructure:

```bash
tofu destroy
```

Note: The state bucket (created by bootstrap) must be destroyed separately:

```bash
cd bootstrap
tofu destroy -var="bucket_name=YOUR_BUCKET_NAME"
```

You may need to empty the bucket first if it contains state files.

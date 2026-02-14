provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project   = "HNDigest"
      ManagedBy = "OpenTofu"
    }
  }
}

variable "aws_region" {
  description = "AWS region for the state bucket"
  type        = string
  default     = "us-west-2"
}

variable "bucket_name" {
  description = "Name for the OpenTofu state bucket (must be globally unique)"
  type        = string
}

variable "project_name" {
  description = "Project name used for resource naming"
  type        = string
  default     = "HNDigest"
}

variable "github_repository" {
  description = "GitHub repository in format owner/repo for OIDC trust"
  type        = string
  default     = "samshadwell/HNDigest"
}

variable "create_github_oidc_provider" {
  description = "Whether to create the GitHub OIDC provider (set to false if it already exists in your account)"
  type        = bool
  default     = true
}

resource "aws_s3_bucket" "tfstate" {
  bucket = var.bucket_name
}

resource "aws_s3_bucket_versioning" "tfstate" {
  bucket = aws_s3_bucket.tfstate.id

  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_public_access_block" "tfstate" {
  bucket = aws_s3_bucket.tfstate.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_server_side_encryption_configuration" "tfstate" {
  bucket = aws_s3_bucket.tfstate.id

  rule {
    apply_server_side_encryption_by_default {
      sse_algorithm = "AES256"
    }
  }
}

###
# High-level config
###
variable "project_name" {
  description = "Project name used for resource naming"
  type        = string
  default     = "HNDigest"
}
variable "aws_region" {
  description = "AWS region for all resources"
  type        = string
  default     = "us-west-2"
}

###
# OpenTofu config
###
variable "github_repository" {
  description = "GitHub repository in format owner/repo for OIDC trust"
  type        = string
  default     = "samshadwell/HNDigest"
}
variable "state_bucket_name" {
  description = "Name of the S3 bucket storing OpenTofu state"
  type        = string
  default     = "hndigest-tfstate"
}
variable "create_github_oidc_provider" {
  description = "Whether to create the GitHub OIDC provider (set to false if it already exists in your account)"
  type        = bool
  default     = true
}

###
# Lambda config
###
variable "lambda_memory_size" {
  description = "Memory size for the Lambda function in MB"
  type        = number
  default     = 256
}
variable "lambda_timeout" {
  description = "Timeout for the Lambda function in seconds"
  type        = number
  default     = 15
}
variable "run_hour_utc" {
  description = "Hour (0-23 UTC) to run the daily digest"
  type        = number
  default     = 5
}

###
# SES config
###
variable "ses_from_email" {
  description = "Email address to send digests from (e.g., hndigest@example.com)"
  type        = string
}
variable "ses_reply_to_email" {
  description = "Reply-to email address (e.g., hello@example.com)"
  type        = string
}
variable "ses_staging_from_email" {
  description = "Email address to send staging digests from (defaults to ses_from_email if not set)"
  type        = string
  default     = null
}

###
# Anti-bot config
###
variable "turnstile_site_key" {
  description = "Cloudflare Turnstile site key (public, embedded in frontend)"
  type        = string
  default     = "0x4AAAAAACTuSJcLuENs4joL"
}

###
# Hosting config
###
variable "landing_page_domain" {
  description = "Domain for the landing page (e.g., hndigest.samshadwell.com)"
  type        = string
  default     = "hndigest.samshadwell.com"
}
variable "landing_page_staging_domain" {
  description = "Staging domain for the landing page (e.g., staging.hndigest.samshadwell.com). If null, no staging alias is created."
  type        = string
  default     = "staging.hndigest.samshadwell.com"
}
variable "landing_page_bucket_name" {
  description = "S3 bucket name for landing page static files"
  type        = string
  default     = "hndigest-landing-page"
}

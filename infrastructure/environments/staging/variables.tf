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

variable "ses_from_email" {
  description = "Email address to send digests from"
  type        = string
}

variable "ses_reply_to_email" {
  description = "Reply-to email address"
  type        = string
}

variable "domain" {
  description = "Domain for the staging landing page"
  type        = string
  default     = "staging.hndigest.samshadwell.com"
}

variable "landing_page_bucket_name" {
  description = "S3 bucket name for landing page static files"
  type        = string
  default     = "hndigest-landing-page-staging"
}

variable "turnstile_site_key" {
  description = "Cloudflare Turnstile site key (public, embedded in frontend)"
  type        = string
  default     = "0x4AAAAAACTuSJcLuENs4joL"
}

variable "cloudfront_web_acl_arn" {
  description = "ARN of the WAF Web ACL created by AWS when enabling CloudFront flat-rate pricing"
  type        = string
  default     = "arn:aws:wafv2:us-east-1:087108798373:global/webacl/CreatedByCloudFront-0cf3e787/f5bd2664-3404-4168-ab68-9f9096a7a065"
}

variable "lambda_memory_size" {
  description = "Memory size for Lambda functions in MB"
  type        = number
  default     = 256
}

variable "lambda_timeout" {
  description = "Timeout for Lambda functions in seconds"
  type        = number
  default     = 15
}

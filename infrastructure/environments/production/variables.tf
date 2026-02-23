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
  default     = "mail@hndigest.samshadwell.com"
}

variable "ses_reply_to_email" {
  description = "Reply-to email address"
  type        = string
  default     = "hi@samshadwell.com"
}

variable "domain" {
  description = "Domain for the production landing page"
  type        = string
  default     = "hndigest.samshadwell.com"
}

variable "landing_page_bucket_name" {
  description = "S3 bucket name for landing page static files"
  type        = string
  default     = "hndigest-landing-page"
}

variable "turnstile_site_key" {
  description = "Cloudflare Turnstile site key (public, embedded in frontend)"
  type        = string
  default     = "0x4AAAAAACTuSJcLuENs4joL"
}

variable "alert_email" {
  description = "Email address for CloudWatch alarm notifications"
  type        = string
  default     = "hi@samshadwell.com"
}

variable "run_hour_utc" {
  description = "Hour (0-23 UTC) to run the daily digest"
  type        = number
  default     = 5
}

variable "cloudfront_web_acl_arn" {
  description = "ARN of the WAF Web ACL created by AWS when enabling CloudFront flat-rate pricing"
  type        = string
  default     = "arn:aws:wafv2:us-east-1:087108798373:global/webacl/CreatedByCloudFront-114a103d/52137a35-f978-4dde-b7eb-2a1eaa1c0fd5"
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

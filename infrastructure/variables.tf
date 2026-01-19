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

variable "lambda_memory_size" {
  description = "Memory size for the Lambda function in MB"
  type        = number
  default     = 256
}

variable "lambda_timeout" {
  description = "Timeout for the Lambda function in seconds"
  type        = number
  default     = 60
}

variable "schedule_expression" {
  description = "EventBridge schedule expression for the digest trigger"
  type        = string
  default     = "cron(0 5 * * ? *)" # Daily at 5:00 AM UTC
}

variable "ses_from_email" {
  description = "Email address to send digests from (e.g., hndigest@example.com)"
  type        = string
}

variable "ses_reply_to_email" {
  description = "Reply-to email address (e.g., hello@example.com)"
  type        = string
}

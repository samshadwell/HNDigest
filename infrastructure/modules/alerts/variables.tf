variable "project_name" {
  description = "Project name used for resource naming"
  type        = string
  default     = "HNDigest"
}

variable "alert_email" {
  description = "Email address for CloudWatch alarm notifications"
  type        = string
}

variable "digest_function_name" {
  description = "Name of the digest Lambda function"
  type        = string
}

variable "bounce_handler_dlq_name" {
  description = "Name of the bounce handler DLQ"
  type        = string
}

variable "api_function_name" {
  description = "Name of the API Lambda function"
  type        = string
}

variable "lambda_timeout" {
  description = "Timeout for Lambda functions in seconds"
  type        = number
}

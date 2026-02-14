variable "project_name" {
  description = "Project name used for resource naming"
  type        = string
  default     = "HNDigest"
}

variable "environment" {
  description = "Environment name (e.g., prod, staging)"
  type        = string
}

variable "name_suffix" {
  description = "Suffix appended to resource names (e.g., '-staging' or '')"
  type        = string
  default     = ""
}

variable "ses_from_email" {
  description = "Email address to send digests from"
  type        = string
}

variable "ses_reply_to_email" {
  description = "Reply-to email address"
  type        = string
}

variable "subject_prefix" {
  description = "Prefix for email subjects (e.g., '[STAGING]' or '')"
  type        = string
  default     = ""
}

variable "base_url" {
  description = "Base URL for the application (e.g., https://hndigest.samshadwell.com)"
  type        = string
}

variable "enable_schedule" {
  description = "Whether to create the EventBridge schedule for daily digest"
  type        = bool
  default     = false
}

variable "run_hour_utc" {
  description = "Hour (0-23 UTC) to run the daily digest"
  type        = number
  default     = 5
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

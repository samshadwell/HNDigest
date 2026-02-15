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

variable "domain" {
  description = "Domain for the landing page (e.g., hndigest.samshadwell.com)"
  type        = string
}

variable "landing_page_bucket_name" {
  description = "S3 bucket name for landing page static files"
  type        = string
}

variable "turnstile_site_key" {
  description = "Cloudflare Turnstile site key (public, embedded in frontend)"
  type        = string
}

variable "static_files_path" {
  description = "Path to the static files directory"
  type        = string
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

variable "ses_from_email" {
  description = "Email address to send from (for API Lambda)"
  type        = string
}

variable "ses_reply_to_email" {
  description = "Reply-to email address (for API Lambda)"
  type        = string
}

# Inputs from digest module
variable "lambda_exec_role_arn" {
  description = "ARN of the Lambda execution IAM role (from digest module)"
  type        = string
}

variable "lambda_exec_role_id" {
  description = "ID of the Lambda execution IAM role (from digest module)"
  type        = string
}

variable "dynamodb_table_name" {
  description = "Name of the DynamoDB table (from digest module)"
  type        = string
}

variable "ses_configuration_set_name" {
  description = "Name of the SES configuration set (from digest module)"
  type        = string
}

variable "kms_ssm_key_arn" {
  description = "ARN of the KMS key used by SSM (from digest module)"
  type        = string
}

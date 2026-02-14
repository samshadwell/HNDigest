output "bucket_name" {
  description = "Name of the S3 bucket for OpenTofu state"
  value       = aws_s3_bucket.tfstate.bucket
}

output "bucket_arn" {
  description = "ARN of the S3 bucket for OpenTofu state"
  value       = aws_s3_bucket.tfstate.arn
}

output "bucket_region" {
  description = "Region of the S3 bucket"
  value       = var.aws_region
}

output "github_actions_role_arn" {
  description = "ARN of the IAM role for GitHub Actions"
  value       = aws_iam_role.github_actions.arn
}

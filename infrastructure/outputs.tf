# Production outputs
output "lambda_function_name" {
  description = "Name of the production Lambda function"
  value       = aws_lambda_function.hndigest["prod"].function_name
}

output "lambda_function_arn" {
  description = "ARN of the production Lambda function"
  value       = aws_lambda_function.hndigest["prod"].arn
}

output "dynamodb_table_name" {
  description = "Name of the production DynamoDB table"
  value       = aws_dynamodb_table.hndigest["prod"].name
}

output "dynamodb_table_arn" {
  description = "ARN of the production DynamoDB table"
  value       = aws_dynamodb_table.hndigest["prod"].arn
}

output "eventbridge_rule_arn" {
  description = "ARN of the EventBridge rule"
  value       = aws_cloudwatch_event_rule.daily_digest["prod"].arn
}

output "github_actions_role_arn" {
  description = "ARN of the IAM role for GitHub Actions"
  value       = aws_iam_role.github_actions.arn
}

# Staging outputs
output "staging_lambda_function_name" {
  description = "Name of the staging Lambda function"
  value       = var.create_staging_environment ? aws_lambda_function.hndigest["staging"].function_name : null
}

output "staging_lambda_function_arn" {
  description = "ARN of the staging Lambda function"
  value       = var.create_staging_environment ? aws_lambda_function.hndigest["staging"].arn : null
}

output "staging_dynamodb_table_name" {
  description = "Name of the staging DynamoDB table"
  value       = var.create_staging_environment ? aws_dynamodb_table.hndigest["staging"].name : null
}

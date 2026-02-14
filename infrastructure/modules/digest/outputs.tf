output "dynamodb_table_name" {
  description = "Name of the DynamoDB table"
  value       = aws_dynamodb_table.hndigest.name
}

output "dynamodb_table_arn" {
  description = "ARN of the DynamoDB table"
  value       = aws_dynamodb_table.hndigest.arn
}

output "lambda_exec_role_arn" {
  description = "ARN of the Lambda execution IAM role"
  value       = aws_iam_role.lambda_exec.arn
}

output "lambda_exec_role_id" {
  description = "ID of the Lambda execution IAM role"
  value       = aws_iam_role.lambda_exec.id
}

output "digest_function_name" {
  description = "Name of the digest Lambda function"
  value       = aws_lambda_function.hndigest.function_name
}

output "bounce_handler_function_name" {
  description = "Name of the bounce handler Lambda function"
  value       = aws_lambda_function.bounce_handler.function_name
}

output "ses_configuration_set_name" {
  description = "Name of the SES configuration set"
  value       = aws_sesv2_configuration_set.main.configuration_set_name
}

output "bounce_handler_dlq_name" {
  description = "Name of the bounce handler DLQ"
  value       = aws_sqs_queue.bounce_handler_dlq.name
}

output "kms_ssm_key_arn" {
  description = "ARN of the KMS key used by SSM SecureString parameters"
  value       = data.aws_kms_alias.ssm.target_key_arn
}

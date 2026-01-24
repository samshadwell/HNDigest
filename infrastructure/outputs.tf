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
  value       = local.create_staging ? aws_lambda_function.hndigest["staging"].function_name : null
}

output "staging_lambda_function_arn" {
  description = "ARN of the staging Lambda function"
  value       = local.create_staging ? aws_lambda_function.hndigest["staging"].arn : null
}

output "staging_dynamodb_table_name" {
  description = "Name of the staging DynamoDB table"
  value       = local.create_staging ? aws_dynamodb_table.hndigest["staging"].name : null
}

# Landing page outputs
output "landing_page_cloudfront_domain" {
  description = "CloudFront distribution domain name for the landing page"
  value       = aws_cloudfront_distribution.landing_page["prod"].domain_name
}

output "landing_page_cloudfront_hosted_zone_id" {
  description = "CloudFront distribution hosted zone ID (for Route53 alias records)"
  value       = aws_cloudfront_distribution.landing_page["prod"].hosted_zone_id
}

output "staging_landing_page_cloudfront_domain" {
  description = "CloudFront distribution domain name for staging"
  value       = local.create_staging ? aws_cloudfront_distribution.landing_page["staging"].domain_name : null
}

output "staging_landing_page_cloudfront_hosted_zone_id" {
  description = "CloudFront distribution hosted zone ID for staging (for Route53 alias records)"
  value       = local.create_staging ? aws_cloudfront_distribution.landing_page["staging"].hosted_zone_id : null
}

output "landing_page_s3_bucket" {
  description = "S3 bucket name for the landing page"
  value       = aws_s3_bucket.landing_page.id
}

output "acm_certificate_validation_records" {
  description = "DNS records needed for ACM certificate validation"
  value = {
    for dvo in aws_acm_certificate.landing_page.domain_validation_options : dvo.domain_name => {
      name  = dvo.resource_record_name
      type  = dvo.resource_record_type
      value = dvo.resource_record_value
    }
  }
}

# API Gateway outputs
output "api_gateway_url" {
  description = "API Gateway endpoint URL (production) - used internally by CloudFront"
  value       = aws_apigatewayv2_api.hndigest["prod"].api_endpoint
}

output "api_lambda_function_name" {
  description = "Name of the API Lambda function (production)"
  value       = aws_lambda_function.hndigest_api["prod"].function_name
}

output "staging_api_lambda_function_name" {
  description = "Name of the staging API Lambda function"
  value       = local.create_staging ? aws_lambda_function.hndigest_api["staging"].function_name : null
}

output "api_function_name" {
  description = "Name of the API Lambda function"
  value       = aws_lambda_function.hndigest_api.function_name
}

output "cloudfront_distribution_id" {
  description = "ID of the CloudFront distribution"
  value       = aws_cloudfront_distribution.landing_page.id
}

output "cloudfront_domain_name" {
  description = "Domain name of the CloudFront distribution"
  value       = aws_cloudfront_distribution.landing_page.domain_name
}

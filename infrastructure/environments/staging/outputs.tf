output "digest_function_name" {
  description = "Name of the digest Lambda function"
  value       = module.digest.digest_function_name
}

output "bounce_handler_function_name" {
  description = "Name of the bounce handler Lambda function"
  value       = module.digest.bounce_handler_function_name
}

output "api_function_name" {
  description = "Name of the API Lambda function"
  value       = module.web.api_function_name
}

output "cloudfront_distribution_id" {
  description = "ID of the CloudFront distribution"
  value       = module.web.cloudfront_distribution_id
}

output "cloudfront_domain_name" {
  description = "Domain name of the CloudFront distribution"
  value       = module.web.cloudfront_domain_name
}

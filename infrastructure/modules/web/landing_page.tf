# Landing page infrastructure
# Static site hosted via S3 + CloudFront

# S3 bucket for static content
resource "aws_s3_bucket" "landing_page" {
  bucket = var.landing_page_bucket_name
}

resource "aws_s3_bucket_public_access_block" "landing_page" {
  bucket = aws_s3_bucket.landing_page.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

# ACM certificate for the landing page domain (must be in us-east-1 for CloudFront)
resource "aws_acm_certificate" "landing_page" {
  provider          = aws.us_east_1
  domain_name       = var.domain
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

# ACM certificate validation
resource "aws_acm_certificate_validation" "landing_page" {
  provider        = aws.us_east_1
  certificate_arn = aws_acm_certificate.landing_page.arn

  timeouts {
    create = "45m"
  }
}

# CloudFront Origin Access Control for S3
resource "aws_cloudfront_origin_access_control" "landing_page" {
  name                              = "${var.landing_page_bucket_name}-oac"
  description                       = "OAC for ${var.project_name} landing page"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

# CloudFront managed cache/origin request policies
data "aws_cloudfront_cache_policy" "caching_optimized" {
  name = "Managed-CachingOptimized"
}
data "aws_cloudfront_cache_policy" "caching_disabled" {
  name = "Managed-CachingDisabled"
}
data "aws_cloudfront_origin_request_policy" "all_viewer_except_host_header" {
  name = "Managed-AllViewerExceptHostHeader"
}

# CloudFront distribution
# NOTE: This uses flat-rate billing with a free tier. This cannot be configured
# in OpenTofu yet, see https://github.com/hashicorp/terraform-provider-aws/issues/45450
resource "aws_cloudfront_distribution" "landing_page" {
  enabled             = true
  is_ipv6_enabled     = true
  default_root_object = "index.html"
  aliases             = [var.domain]
  price_class         = "PriceClass_All"
  web_acl_id          = var.cloudfront_web_acl_arn

  # S3 origin for static content
  origin {
    domain_name              = aws_s3_bucket.landing_page.bucket_regional_domain_name
    origin_id                = "S3-landing-page"
    origin_access_control_id = aws_cloudfront_origin_access_control.landing_page.id
  }

  # API Gateway origin for /api/* requests
  origin {
    domain_name = replace(aws_apigatewayv2_api.hndigest.api_endpoint, "https://", "")
    origin_id   = "APIGateway"

    custom_origin_config {
      http_port              = 80
      https_port             = 443
      origin_protocol_policy = "https-only"
      origin_ssl_protocols   = ["TLSv1.2"]
    }
  }

  # Default behavior: serve from S3
  default_cache_behavior {
    allowed_methods        = ["GET", "HEAD", "OPTIONS"]
    cached_methods         = ["GET", "HEAD"]
    target_origin_id       = "S3-landing-page"
    viewer_protocol_policy = "redirect-to-https"
    compress               = true
    cache_policy_id        = data.aws_cloudfront_cache_policy.caching_optimized.id
  }

  # API behavior: forward /api/* to API Gateway
  ordered_cache_behavior {
    path_pattern             = "/api/*"
    allowed_methods          = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"]
    cached_methods           = ["GET", "HEAD"]
    target_origin_id         = "APIGateway"
    viewer_protocol_policy   = "redirect-to-https"
    compress                 = true
    cache_policy_id          = data.aws_cloudfront_cache_policy.caching_disabled.id
    origin_request_policy_id = data.aws_cloudfront_origin_request_policy.all_viewer_except_host_header.id
  }

  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }

  viewer_certificate {
    acm_certificate_arn      = aws_acm_certificate_validation.landing_page.certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }

  custom_error_response {
    error_code         = 404
    response_code      = 200
    response_page_path = "/index.html"
  }

  depends_on = [aws_acm_certificate_validation.landing_page]
}

# S3 bucket policy allowing CloudFront access
resource "aws_s3_bucket_policy" "landing_page" {
  bucket = aws_s3_bucket.landing_page.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid    = "AllowCloudFrontServicePrincipal"
        Effect = "Allow"
        Principal = {
          Service = "cloudfront.amazonaws.com"
        }
        Action   = "s3:GetObject"
        Resource = "${aws_s3_bucket.landing_page.arn}/*"
        Condition = {
          StringEquals = {
            "AWS:SourceArn" = aws_cloudfront_distribution.landing_page.arn
          }
        }
      }
    ]
  })
}

# Content type mapping by file extension
locals {
  content_types = {
    ".html" = "text/html"
    ".css"  = "text/css"
    ".js"   = "application/javascript"
    ".json" = "application/json"
    ".png"  = "image/png"
    ".jpg"  = "image/jpeg"
    ".jpeg" = "image/jpeg"
    ".gif"  = "image/gif"
    ".svg"  = "image/svg+xml"
    ".ico"  = "image/x-icon"
    ".txt"  = "text/plain"
  }
}

# Upload static files (except index.html which is templated separately)
resource "aws_s3_object" "static_files" {
  for_each = { for f in fileset(var.static_files_path, "**/*") : f => f if f != "index.html" }

  bucket       = aws_s3_bucket.landing_page.id
  key          = each.value
  source       = "${var.static_files_path}/${each.value}"
  content_type = lookup(local.content_types, regex("\\.[^.]+$", each.value), "application/octet-stream")
  etag         = filemd5("${var.static_files_path}/${each.value}")
}

# index.html is templated to inject the Turnstile site key
resource "aws_s3_object" "index_html" {
  bucket = aws_s3_bucket.landing_page.id
  key    = "index.html"
  content = templatefile("${var.static_files_path}/index.html", {
    turnstile_site_key = var.turnstile_site_key
  })
  content_type = "text/html"
  etag = md5(templatefile("${var.static_files_path}/index.html", {
    turnstile_site_key = var.turnstile_site_key
  }))
}

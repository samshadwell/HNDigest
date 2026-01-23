# Landing page infrastructure
# Static site hosted via S3 + CloudFront

locals {
  # Include staging domain if set, otherwise just production
  landing_page_aliases = compact([
    var.landing_page_domain,
    var.landing_page_staging_domain
  ])
}

# ACM certificate must be in us-east-1 for CloudFront
provider "aws" {
  alias  = "us_east_1"
  region = "us-east-1"

  default_tags {
    tags = {
      Project   = var.project_name
      ManagedBy = "OpenTofu"
    }
  }
}

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

# ACM certificate for the landing page domain
resource "aws_acm_certificate" "landing_page" {
  provider          = aws.us_east_1
  domain_name       = var.landing_page_domain
  validation_method = "DNS"

  # Also cover wildcard for subdomains (api, staging, etc.)
  subject_alternative_names = [
    "*.${var.landing_page_domain}"
  ]

  lifecycle {
    create_before_destroy = true
  }
}

# ACM certificate validation
# Terraform will wait here until DNS records are added and certificate validates
resource "aws_acm_certificate_validation" "landing_page" {
  provider        = aws.us_east_1
  certificate_arn = aws_acm_certificate.landing_page.arn

  timeouts {
    create = "45m" # Give time to add DNS records
  }
}

# CloudFront Origin Access Control for S3
resource "aws_cloudfront_origin_access_control" "landing_page" {
  name                              = "hndigest-landing-page-oac"
  description                       = "OAC for HNDigest landing page"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

# CloudFront distribution
resource "aws_cloudfront_distribution" "landing_page" {
  enabled             = true
  is_ipv6_enabled     = true
  default_root_object = "index.html"
  aliases             = local.landing_page_aliases
  price_class         = "PriceClass_100" # US, Canada, Europe only (cheapest)

  origin {
    domain_name              = aws_s3_bucket.landing_page.bucket_regional_domain_name
    origin_id                = "S3-landing-page"
    origin_access_control_id = aws_cloudfront_origin_access_control.landing_page.id
  }

  default_cache_behavior {
    allowed_methods        = ["GET", "HEAD", "OPTIONS"]
    cached_methods         = ["GET", "HEAD"]
    target_origin_id       = "S3-landing-page"
    viewer_protocol_policy = "redirect-to-https"
    compress               = true

    forwarded_values {
      query_string = false
      cookies {
        forward = "none"
      }
    }

    min_ttl     = 0
    default_ttl = 3600  # 1 hour
    max_ttl     = 86400 # 24 hours
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

# Upload the index.html file
resource "aws_s3_object" "index_html" {
  bucket       = aws_s3_bucket.landing_page.id
  key          = "index.html"
  source       = "${path.module}/../static/index.html"
  content_type = "text/html"
  etag         = filemd5("${path.module}/../static/index.html")
}

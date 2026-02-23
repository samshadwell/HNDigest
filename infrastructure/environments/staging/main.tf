provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = var.project_name
      Environment = "staging"
      ManagedBy   = "OpenTofu"
    }
  }
}

provider "aws" {
  alias  = "us_east_1"
  region = "us-east-1"

  default_tags {
    tags = {
      Project     = var.project_name
      Environment = "staging"
      ManagedBy   = "OpenTofu"
    }
  }
}

module "digest" {
  source = "../../modules/digest"

  environment        = "staging"
  name_suffix        = "-staging"
  ses_from_email     = var.ses_from_email
  ses_reply_to_email = var.ses_reply_to_email
  subject_prefix     = "[STAGING]"
  base_url           = "https://${var.domain}"
  enable_schedule    = false
  lambda_memory_size = var.lambda_memory_size
  lambda_timeout     = var.lambda_timeout
}

module "web" {
  source = "../../modules/web"

  providers = {
    aws           = aws
    aws.us_east_1 = aws.us_east_1
  }

  environment              = "staging"
  name_suffix              = "-staging"
  cloudfront_web_acl_arn   = var.cloudfront_web_acl_arn
  domain                   = var.domain
  landing_page_bucket_name = var.landing_page_bucket_name
  turnstile_site_key       = var.turnstile_site_key
  static_files_path        = "${path.module}/../../../static"
  ses_from_email           = var.ses_from_email
  ses_reply_to_email       = var.ses_reply_to_email
  lambda_memory_size       = var.lambda_memory_size
  lambda_timeout           = var.lambda_timeout

  lambda_exec_role_arn       = module.digest.lambda_exec_role_arn
  lambda_exec_role_id        = module.digest.lambda_exec_role_id
  dynamodb_table_name        = module.digest.dynamodb_table_name
  ses_configuration_set_name = module.digest.ses_configuration_set_name
  kms_ssm_key_arn            = module.digest.kms_ssm_key_arn
}

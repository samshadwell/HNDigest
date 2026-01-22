terraform {
  required_version = ">= 1.6.0"

  backend "s3" {
    bucket       = "hndigest-tfstate"
    key          = "infrastructure/terraform.tfstate"
    region       = "us-west-2"
    use_lockfile = true
  }

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.28"
    }
    archive = {
      source  = "hashicorp/archive"
      version = "~> 2.0"
    }
  }
}

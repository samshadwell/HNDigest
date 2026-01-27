# SSM Parameter Store for secrets
# Values are managed outside of Terraform (via AWS Console or CLI)
# to keep them out of the state file.

resource "aws_ssm_parameter" "turnstile_secret_key" {
  for_each = local.environments

  name  = "/${var.project_name}/${each.key}/turnstile-secret-key"
  type  = "SecureString"
  value = "REPLACE_ME" # Set real value via: aws ssm put-parameter --name <name> --value <secret> --type SecureString --overwrite

  lifecycle {
    ignore_changes = [value]
  }
}

# The default KMS key used by SSM SecureString parameters
data "aws_kms_alias" "ssm" {
  name = "alias/aws/ssm"
}


# SSM Parameter Store for secrets
# Values are managed outside of Terraform (via AWS Console or CLI)
# to keep them out of the state file.

resource "aws_ssm_parameter" "turnstile_secret_key" {
  name  = "/${var.project_name}/${var.environment}/turnstile-secret-key"
  type  = "SecureString"
  value = "REPLACE_ME" # Set real value via: aws ssm put-parameter --name <name> --value <secret> --type SecureString --overwrite

  lifecycle {
    ignore_changes = [value]
  }
}

# SSM Parameter Store access (for secrets) - attached to the role from digest module
resource "aws_iam_role_policy" "ssm_access" {
  name = "${lower(var.project_name)}${var.name_suffix}-ssm-access"
  role = var.lambda_exec_role_id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "ssm:GetParameter"
        ]
        Resource = aws_ssm_parameter.turnstile_secret_key.arn
      },
      {
        Effect = "Allow"
        Action = [
          "kms:Decrypt"
        ]
        Resource = var.kms_ssm_key_arn
      }
    ]
  })
}

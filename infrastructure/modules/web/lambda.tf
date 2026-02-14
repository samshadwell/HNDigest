# Placeholder zip for Lambda creation - actual code is deployed via CI/CD
data "archive_file" "lambda_placeholder" {
  type        = "zip"
  output_path = "${path.module}/.placeholder.zip"

  source {
    content  = "#!/bin/sh\necho 'Placeholder - deploy real code via CI/CD'"
    filename = "bootstrap"
  }
}

# API Lambda function (separate from digest Lambda)
resource "aws_lambda_function" "hndigest_api" {
  function_name = "${var.project_name}${var.name_suffix}-api"
  role          = var.lambda_exec_role_arn
  handler       = "bootstrap"
  runtime       = "provided.al2023"
  architectures = ["x86_64"]

  filename         = data.archive_file.lambda_placeholder.output_path
  source_code_hash = data.archive_file.lambda_placeholder.output_base64sha256

  memory_size = var.lambda_memory_size
  timeout     = var.lambda_timeout

  environment {
    variables = {
      AWS_LAMBDA_LOG_FORMAT      = "json"
      RUST_LOG                   = "info"
      DYNAMODB_TABLE             = var.dynamodb_table_name
      BASE_URL                   = "https://${var.domain}"
      EMAIL_FROM                 = var.ses_from_email
      EMAIL_REPLY_TO             = var.ses_reply_to_email
      TURNSTILE_SECRET_KEY_PARAM = aws_ssm_parameter.turnstile_secret_key.name
      SES_CONFIGURATION_SET      = var.ses_configuration_set_name
    }
  }

  lifecycle {
    ignore_changes = [
      filename,
      source_code_hash,
    ]
  }
}

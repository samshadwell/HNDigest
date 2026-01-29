# Placeholder zip for Lambda creation - actual code is deployed via CI/CD
data "archive_file" "lambda_placeholder" {
  type        = "zip"
  output_path = "${path.module}/.placeholder.zip"

  source {
    content  = "#!/bin/sh\necho 'Placeholder - deploy real code via CI/CD'"
    filename = "bootstrap"
  }
}

resource "aws_lambda_function" "hndigest" {
  for_each = local.environments

  function_name = each.value.function_name
  role          = aws_iam_role.lambda_exec[each.key].arn
  handler       = "bootstrap"
  runtime       = "provided.al2023"
  architectures = ["x86_64"]

  filename         = data.archive_file.lambda_placeholder.output_path
  source_code_hash = data.archive_file.lambda_placeholder.output_base64sha256

  memory_size = var.lambda_memory_size
  timeout     = var.lambda_timeout

  environment {
    variables = merge(
      {
        AWS_LAMBDA_LOG_FORMAT = "json"
        RUST_LOG              = "info"
        DYNAMODB_TABLE        = each.value.table_name
        EMAIL_FROM            = each.value.from_email
        EMAIL_REPLY_TO        = each.value.reply_to_email
        RUN_HOUR_UTC          = tostring(var.run_hour_utc)
        BASE_URL              = "https://${each.value.domain}"
        SES_CONFIGURATION_SET = aws_sesv2_configuration_set.main[each.key].configuration_set_name
      },
      each.value.subject_prefix != "" ? { SUBJECT_PREFIX = each.value.subject_prefix } : {}
    )
  }

  # Infrastructure is managed by OpenTofu, but code deployment
  # happens separately via CI/CD
  lifecycle {
    ignore_changes = [
      filename,
      source_code_hash,
    ]
  }
}

# API Lambda function (separate from digest Lambda)
resource "aws_lambda_function" "hndigest_api" {
  for_each = local.environments

  function_name = "${each.value.function_name}-api"
  role          = aws_iam_role.lambda_exec[each.key].arn
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
      DYNAMODB_TABLE             = each.value.table_name
      BASE_URL                   = "https://${each.value.domain}"
      EMAIL_FROM                 = each.value.from_email
      EMAIL_REPLY_TO             = each.value.reply_to_email
      TURNSTILE_SECRET_KEY_PARAM = aws_ssm_parameter.turnstile_secret_key[each.key].name
      SES_CONFIGURATION_SET      = aws_sesv2_configuration_set.main[each.key].configuration_set_name
    }
  }

  lifecycle {
    ignore_changes = [
      filename,
      source_code_hash,
    ]
  }
}

# Bounce/complaint handler Lambda (triggered by SNS)
resource "aws_lambda_function" "bounce_handler" {
  for_each = local.environments

  function_name = "${each.value.function_name}-bounce-handler"
  role          = aws_iam_role.lambda_exec[each.key].arn
  handler       = "bootstrap"
  runtime       = "provided.al2023"
  architectures = ["x86_64"]

  filename         = data.archive_file.lambda_placeholder.output_path
  source_code_hash = data.archive_file.lambda_placeholder.output_base64sha256

  memory_size = var.lambda_memory_size
  timeout     = var.lambda_timeout

  dead_letter_config {
    target_arn = aws_sqs_queue.bounce_handler_dlq[each.key].arn
  }

  environment {
    variables = {
      AWS_LAMBDA_LOG_FORMAT = "json"
      RUST_LOG              = "info"
      DYNAMODB_TABLE        = each.value.table_name
    }
  }

  lifecycle {
    ignore_changes = [
      filename,
      source_code_hash,
    ]
  }
}

# Allow SNS to invoke the bounce handler Lambda
resource "aws_lambda_permission" "sns_bounce_handler" {
  for_each = local.environments

  statement_id  = "AllowSNSInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.bounce_handler[each.key].function_name
  principal     = "sns.amazonaws.com"
  source_arn    = aws_sns_topic.ses_notifications[each.key].arn
}

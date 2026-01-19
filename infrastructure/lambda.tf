# Placeholder zip for Lambda creation - actual code is deployed via deploy.sh
data "archive_file" "lambda_placeholder" {
  type        = "zip"
  output_path = "${path.module}/.placeholder.zip"

  source {
    content  = "#!/bin/sh\necho 'Placeholder - deploy real code via deploy.sh'"
    filename = "bootstrap"
  }
}

resource "aws_lambda_function" "hndigest" {
  function_name = var.project_name
  role          = aws_iam_role.lambda_exec.arn
  handler       = "bootstrap"
  runtime       = "provided.al2023"
  architectures = ["x86_64"]

  filename         = data.archive_file.lambda_placeholder.output_path
  source_code_hash = data.archive_file.lambda_placeholder.output_base64sha256

  memory_size = var.lambda_memory_size
  timeout     = var.lambda_timeout

  environment {
    variables = {
      RUST_LOG = "info"
    }
  }

  # Infrastructure is managed by OpenTofu, but code deployment
  # happens separately via deploy.sh
  lifecycle {
    ignore_changes = [
      filename,
      source_code_hash,
    ]
  }
}

# Permission for EventBridge to invoke Lambda
resource "aws_lambda_permission" "eventbridge" {
  statement_id  = "AllowEventBridgeInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.hndigest.function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.daily_digest.arn
}

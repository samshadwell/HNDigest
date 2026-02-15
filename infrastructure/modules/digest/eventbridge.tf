resource "aws_cloudwatch_event_rule" "daily_digest" {
  count = var.enable_schedule ? 1 : 0

  name                = "${var.project_name}${var.name_suffix}-trigger"
  description         = "Triggers ${var.project_name}${var.name_suffix} Lambda daily"
  schedule_expression = "cron(0 ${var.run_hour_utc} * * ? *)"
}

resource "aws_cloudwatch_event_target" "lambda" {
  count = var.enable_schedule ? 1 : 0

  rule      = aws_cloudwatch_event_rule.daily_digest[0].name
  target_id = "${var.project_name}${var.name_suffix}Lambda"
  arn       = aws_lambda_function.hndigest.arn
}

resource "aws_lambda_permission" "eventbridge" {
  count = var.enable_schedule ? 1 : 0

  statement_id  = "AllowEventBridgeInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.hndigest.function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.daily_digest[0].arn
}

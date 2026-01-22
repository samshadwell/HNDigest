locals {
  scheduled_environments = { for k, v in local.environments : k => v if v.has_schedule }
}

resource "aws_cloudwatch_event_rule" "daily_digest" {
  for_each = local.scheduled_environments

  name                = "${each.value.function_name}-trigger"
  description         = "Triggers ${each.value.function_name} Lambda daily"
  schedule_expression = "cron(0 ${var.run_hour_utc} * * ? *)"
}

resource "aws_cloudwatch_event_target" "lambda" {
  for_each = local.scheduled_environments

  rule      = aws_cloudwatch_event_rule.daily_digest[each.key].name
  target_id = "${each.value.function_name}Lambda"
  arn       = aws_lambda_function.hndigest[each.key].arn
}

resource "aws_lambda_permission" "eventbridge" {
  for_each = local.scheduled_environments

  statement_id  = "AllowEventBridgeInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.hndigest[each.key].function_name
  principal     = "events.amazonaws.com"
  source_arn    = aws_cloudwatch_event_rule.daily_digest[each.key].arn
}

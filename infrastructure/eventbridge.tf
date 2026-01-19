resource "aws_cloudwatch_event_rule" "daily_digest" {
  name                = "${var.project_name}-trigger"
  description         = "Triggers ${var.project_name} Lambda daily at 5:00 AM UTC"
  schedule_expression = var.schedule_expression
}

resource "aws_cloudwatch_event_target" "lambda" {
  rule      = aws_cloudwatch_event_rule.daily_digest.name
  target_id = "${var.project_name}Lambda"
  arn       = aws_lambda_function.hndigest.arn
}

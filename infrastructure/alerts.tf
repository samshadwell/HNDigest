locals {
  alerted_environments = { for k, v in local.environments : k => v if v.has_alerts }
}

# SNS topic for CloudWatch alarm notifications
resource "aws_sns_topic" "alerts" {
  for_each = local.alerted_environments

  name = "${lower(var.project_name)}-alerts"
}

resource "aws_sns_topic_subscription" "alerts_email" {
  for_each = local.alerted_environments

  topic_arn = aws_sns_topic.alerts[each.key].arn
  protocol  = "email"
  endpoint  = var.alert_email
}

# Metric filter: detect "Subscription verified successfully" in API Lambda logs
resource "aws_cloudwatch_log_metric_filter" "subscription_verified" {
  for_each = local.alerted_environments

  name           = "${lower(var.project_name)}-subscription-verified"
  pattern        = "\"Subscription verified successfully\""
  log_group_name = "/aws/lambda/${aws_lambda_function.hndigest_api[each.key].function_name}"

  metric_transformation {
    name      = "SubscriptionVerified"
    namespace = "${var.project_name}/Alerts"
    value     = "1"
  }
}

###
# Alarm 1: DLQ not empty
###
resource "aws_cloudwatch_metric_alarm" "dlq_not_empty" {
  for_each = local.alerted_environments

  alarm_name          = "${lower(var.project_name)}-dlq-not-empty"
  alarm_description   = "Bounce handler DLQ has messages - unprocessed SES notifications"
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 1
  metric_name         = "ApproximateNumberOfMessagesVisible"
  namespace           = "AWS/SQS"
  period              = 60
  statistic           = "Maximum"
  threshold           = 1
  treat_missing_data  = "notBreaching"

  dimensions = {
    QueueName = aws_sqs_queue.bounce_handler_dlq[each.key].name
  }

  alarm_actions = [aws_sns_topic.alerts[each.key].arn]
  ok_actions    = [aws_sns_topic.alerts[each.key].arn]
}

###
# Alarm 2: Subscription verified
###
resource "aws_cloudwatch_metric_alarm" "subscription_verified" {
  for_each = local.alerted_environments

  alarm_name          = "${lower(var.project_name)}-subscription-verified"
  alarm_description   = "A new email subscription was verified"
  comparison_operator = "GreaterThanOrEqualToThreshold"
  evaluation_periods  = 1
  metric_name         = "SubscriptionVerified"
  namespace           = "${var.project_name}/Alerts"
  period              = 60
  statistic           = "Sum"
  threshold           = 1
  treat_missing_data  = "notBreaching"

  alarm_actions = [aws_sns_topic.alerts[each.key].arn]
}

###
# Alarm 3: Digest not invoked (dead man's switch)
###
resource "aws_cloudwatch_metric_alarm" "digest_not_invoked" {
  for_each = local.alerted_environments

  alarm_name          = "${lower(var.project_name)}-digest-not-invoked"
  alarm_description   = "Digest Lambda has not been invoked in 24 hours"
  comparison_operator = "LessThanThreshold"
  evaluation_periods  = 1
  metric_name         = "Invocations"
  namespace           = "AWS/Lambda"
  period              = 86400
  statistic           = "Sum"
  threshold           = 1
  treat_missing_data  = "breaching"

  dimensions = {
    FunctionName = aws_lambda_function.hndigest[each.key].function_name
  }

  alarm_actions = [aws_sns_topic.alerts[each.key].arn]
  ok_actions    = [aws_sns_topic.alerts[each.key].arn]
}

###
# Alarm 4: API error rate > 50%
###
resource "aws_cloudwatch_metric_alarm" "api_error_rate" {
  for_each = local.alerted_environments

  alarm_name          = "${lower(var.project_name)}-api-error-rate"
  alarm_description   = "API Lambda error rate exceeds 50% for 15 minutes"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 3
  threshold           = 50
  treat_missing_data  = "notBreaching"

  metric_query {
    id          = "error_rate"
    expression  = "IF(invocations > 0, (errors / invocations) * 100, 0)"
    label       = "Error Rate (%)"
    return_data = true
  }

  metric_query {
    id = "errors"

    metric {
      metric_name = "Errors"
      namespace   = "AWS/Lambda"
      period      = 300
      stat        = "Sum"

      dimensions = {
        FunctionName = aws_lambda_function.hndigest_api[each.key].function_name
      }
    }
  }

  metric_query {
    id = "invocations"

    metric {
      metric_name = "Invocations"
      namespace   = "AWS/Lambda"
      period      = 300
      stat        = "Sum"

      dimensions = {
        FunctionName = aws_lambda_function.hndigest_api[each.key].function_name
      }
    }
  }

  alarm_actions = [aws_sns_topic.alerts[each.key].arn]
  ok_actions    = [aws_sns_topic.alerts[each.key].arn]
}

###
# Alarm 5: Digest duration high (>80% of timeout)
###
resource "aws_cloudwatch_metric_alarm" "digest_duration_high" {
  for_each = local.alerted_environments

  alarm_name          = "${lower(var.project_name)}-digest-duration-high"
  alarm_description   = "Digest Lambda max duration exceeds 80% of timeout (${var.lambda_timeout}s)"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = 1
  metric_name         = "Duration"
  namespace           = "AWS/Lambda"
  period              = 300
  statistic           = "Maximum"
  threshold           = var.lambda_timeout * 1000 * 0.8
  treat_missing_data  = "notBreaching"

  dimensions = {
    FunctionName = aws_lambda_function.hndigest[each.key].function_name
  }

  alarm_actions = [aws_sns_topic.alerts[each.key].arn]
  ok_actions    = [aws_sns_topic.alerts[each.key].arn]
}

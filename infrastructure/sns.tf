# SNS topic for SES bounce/complaint notifications
resource "aws_sns_topic" "ses_notifications" {
  for_each = local.environments

  name = "${lower(var.project_name)}${each.value.name_suffix}-ses-notifications"
}

# Allow SES to publish to the SNS topic
resource "aws_sns_topic_policy" "ses_notifications" {
  for_each = local.environments

  arn = aws_sns_topic.ses_notifications[each.key].arn

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect    = "Allow"
        Principal = { Service = "ses.amazonaws.com" }
        Action    = "sns:Publish"
        Resource  = aws_sns_topic.ses_notifications[each.key].arn
        Condition = {
          StringEquals = {
            "AWS:SourceAccount" = data.aws_caller_identity.current.account_id
          }
        }
      }
    ]
  })
}

# Subscribe the bounce handler Lambda to the SNS topic
resource "aws_sns_topic_subscription" "bounce_handler" {
  for_each = local.environments

  topic_arn = aws_sns_topic.ses_notifications[each.key].arn
  protocol  = "lambda"
  endpoint  = aws_lambda_function.bounce_handler[each.key].arn
}

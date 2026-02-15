data "aws_caller_identity" "current" {}

# SNS topic for SES bounce/complaint notifications
resource "aws_sns_topic" "ses_notifications" {
  name = "${lower(var.project_name)}${var.name_suffix}-ses-notifications"
}

# Allow SES to publish to the SNS topic
resource "aws_sns_topic_policy" "ses_notifications" {
  arn = aws_sns_topic.ses_notifications.arn

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect    = "Allow"
        Principal = { Service = "ses.amazonaws.com" }
        Action    = "sns:Publish"
        Resource  = aws_sns_topic.ses_notifications.arn
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
  topic_arn = aws_sns_topic.ses_notifications.arn
  protocol  = "lambda"
  endpoint  = aws_lambda_function.bounce_handler.arn

  redrive_policy = jsonencode({
    deadLetterTargetArn = aws_sqs_queue.bounce_handler_dlq.arn
  })
}

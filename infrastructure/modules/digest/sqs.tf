# Dead-letter queue for bounce handler Lambda
# Failed events land here for manual inspection
resource "aws_sqs_queue" "bounce_handler_dlq" {
  name                      = "${lower(var.project_name)}${var.name_suffix}-bounce-handler-dlq"
  message_retention_seconds = 1209600 # 14 days
}

# Allow SNS to send undeliverable messages to the DLQ
resource "aws_sqs_queue_policy" "bounce_handler_dlq" {
  queue_url = aws_sqs_queue.bounce_handler_dlq.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect    = "Allow"
        Principal = { Service = "sns.amazonaws.com" }
        Action    = "sqs:SendMessage"
        Resource  = aws_sqs_queue.bounce_handler_dlq.arn
        Condition = {
          ArnEquals = {
            "aws:SourceArn" = aws_sns_topic.ses_notifications.arn
          }
        }
      }
    ]
  })
}

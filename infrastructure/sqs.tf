# Dead-letter queue for bounce handler Lambda
# Failed events land here for manual inspection
resource "aws_sqs_queue" "bounce_handler_dlq" {
  for_each = local.environments

  name                      = "${lower(var.project_name)}${each.value.name_suffix}-bounce-handler-dlq"
  message_retention_seconds = 1209600 # 14 days
}

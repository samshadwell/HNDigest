resource "aws_ses_domain_identity" "sender" {
  domain = local.ses_domain
}

# SES configuration set for tracking bounces and complaints
resource "aws_sesv2_configuration_set" "main" {
  for_each = local.environments

  configuration_set_name = "${lower(var.project_name)}${each.value.name_suffix}"

  reputation_options {
    reputation_metrics_enabled = true
  }
}

# Route bounce and complaint events to SNS
resource "aws_sesv2_configuration_set_event_destination" "sns" {
  for_each = local.environments

  configuration_set_name = aws_sesv2_configuration_set.main[each.key].configuration_set_name
  event_destination_name = "bounce-complaint-sns"

  event_destination {
    enabled              = true
    matching_event_types = ["BOUNCE", "COMPLAINT"]

    sns_destination {
      topic_arn = aws_sns_topic.ses_notifications[each.key].arn
    }
  }
}

locals {
  # Derive SES domain from the from_email address
  ses_domain = split("@", var.ses_from_email)[1]

  # Staging is enabled if a staging domain is configured
  create_staging = var.landing_page_staging_domain != null

  # Environment definitions - each environment gets its own Lambda, DynamoDB, and IAM role
  environments = merge(
    {
      prod = {
        name_suffix    = ""
        table_name     = var.project_name
        function_name  = var.project_name
        role_name      = "${lower(var.project_name)}-lambda-role"
        from_email     = var.ses_from_email
        reply_to_email = var.ses_reply_to_email
        subject_prefix = ""
        # Prod gets the EventBridge schedule and alerts
        has_schedule = true
        has_alerts   = true
        domain       = var.landing_page_domain
      }
    },
    local.create_staging ? {
      staging = {
        name_suffix    = "-staging"
        table_name     = "${var.project_name}-staging"
        function_name  = "${var.project_name}-staging"
        role_name      = "${lower(var.project_name)}-staging-lambda-role"
        from_email     = coalesce(var.ses_staging_from_email, var.ses_from_email)
        reply_to_email = var.ses_reply_to_email
        subject_prefix = "[STAGING]"
        # Staging is triggered manually, no schedule or alerts
        has_schedule = false
        has_alerts   = false
        domain       = var.landing_page_staging_domain
      }
    } : {}
  )
}

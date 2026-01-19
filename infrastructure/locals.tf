locals {
  # Derive SES domain from the from_email address
  ses_domain = split("@", var.ses_from_email)[1]
}

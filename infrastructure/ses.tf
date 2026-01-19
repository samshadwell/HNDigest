resource "aws_ses_domain_identity" "sender" {
  domain = local.ses_domain
}

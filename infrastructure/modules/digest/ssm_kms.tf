# The default KMS key used by SSM SecureString parameters
data "aws_kms_alias" "ssm" {
  name = "alias/aws/ssm"
}

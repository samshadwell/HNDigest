resource "aws_dynamodb_table" "hndigest" {
  name         = "${var.project_name}${var.name_suffix}"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "PK"
  range_key    = "SK"

  attribute {
    name = "PK"
    type = "S"
  }

  attribute {
    name = "SK"
    type = "S"
  }

  attribute {
    name = "unsubscribe_token"
    type = "S"
  }

  # GSI for looking up subscribers by their unsubscribe token
  global_secondary_index {
    name = "unsubscribe_token_index"
    key_schema {
      attribute_name = "unsubscribe_token"
      key_type       = "HASH"
    }
    projection_type = "ALL"
  }

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }
}

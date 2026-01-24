resource "aws_dynamodb_table" "hndigest" {
  for_each = local.environments

  name         = each.value.table_name
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
    name            = "unsubscribe_token_index"
    hash_key        = "unsubscribe_token"
    projection_type = "ALL"
  }

  ttl {
    attribute_name = "expires_at"
    enabled        = true
  }
}

# IAM role for Lambda execution (per environment)
resource "aws_iam_role" "lambda_exec" {
  for_each = local.environments

  name = each.value.role_name

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "lambda.amazonaws.com"
        }
      }
    ]
  })
}

# CloudWatch Logs policy
resource "aws_iam_role_policy_attachment" "lambda_logs" {
  for_each = local.environments

  role       = aws_iam_role.lambda_exec[each.key].name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

# DynamoDB access policy
resource "aws_iam_role_policy" "dynamodb_access" {
  for_each = local.environments

  name = "${lower(var.project_name)}${each.value.name_suffix}-dynamodb-access"
  role = aws_iam_role.lambda_exec[each.key].id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "dynamodb:BatchGetItem",
          "dynamodb:GetItem",
          "dynamodb:PutItem",
          "dynamodb:UpdateItem",
          "dynamodb:DeleteItem",
          "dynamodb:Query",
          "dynamodb:Scan"
        ]
        Resource = [
          aws_dynamodb_table.hndigest[each.key].arn,
          "${aws_dynamodb_table.hndigest[each.key].arn}/index/*"
        ]
      }
    ]
  })
}

# SES sending policy
resource "aws_iam_role_policy" "ses_access" {
  for_each = local.environments

  name = "${lower(var.project_name)}${each.value.name_suffix}-ses-access"
  role = aws_iam_role.lambda_exec[each.key].id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "ses:SendEmail",
          "ses:SendRawEmail"
        ]
        Resource = "*"
        Condition = {
          StringEquals = {
            "ses:FromAddress" = each.value.from_email
          }
        }
      }
    ]
  })
}

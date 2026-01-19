# GitHub Actions OIDC provider
data "aws_iam_openid_connect_provider" "github" {
  count = var.create_github_oidc_provider ? 0 : 1
  url   = "https://token.actions.githubusercontent.com"
}

resource "aws_iam_openid_connect_provider" "github" {
  count           = var.create_github_oidc_provider ? 1 : 0
  url             = "https://token.actions.githubusercontent.com"
  client_id_list  = ["sts.amazonaws.com"]
  thumbprint_list = ["d89e3bd43d5d909b47a18977aa9d5ce36cee184c"]
}

locals {
  github_oidc_provider_arn = var.create_github_oidc_provider ? aws_iam_openid_connect_provider.github[0].arn : data.aws_iam_openid_connect_provider.github[0].arn
}

# IAM role for GitHub Actions
resource "aws_iam_role" "github_actions" {
  name = "${lower(var.project_name)}-github-actions"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Principal = {
          Federated = local.github_oidc_provider_arn
        }
        Action = "sts:AssumeRoleWithWebIdentity"
        Condition = {
          StringEquals = {
            "token.actions.githubusercontent.com:aud" = "sts.amazonaws.com"
          }
          StringLike = {
            "token.actions.githubusercontent.com:sub" = "repo:${var.github_repository}:*"
          }
        }
      }
    ]
  })
}

# Policy for OpenTofu state access
resource "aws_iam_role_policy" "github_actions_state" {
  name = "${lower(var.project_name)}-github-actions-state"
  role = aws_iam_role.github_actions.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "s3:GetObject",
          "s3:PutObject",
          "s3:DeleteObject",
          "s3:ListBucket"
        ]
        Resource = [
          "arn:aws:s3:::${var.state_bucket_name}",
          "arn:aws:s3:::${var.state_bucket_name}/*"
        ]
      }
    ]
  })
}

# Policy for managing infrastructure resources
# Uses action wildcards with resource restrictions - limits what resources
# can be affected rather than enumerating every possible action
resource "aws_iam_role_policy" "github_actions_infra" {
  name = "${lower(var.project_name)}-github-actions-infra"
  role = aws_iam_role.github_actions.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid      = "Lambda"
        Effect   = "Allow"
        Action   = "lambda:*"
        Resource = aws_lambda_function.hndigest.arn
      },
      {
        Sid      = "DynamoDB"
        Effect   = "Allow"
        Action   = "dynamodb:*"
        Resource = aws_dynamodb_table.hndigest.arn
      },
      {
        Sid      = "EventBridge"
        Effect   = "Allow"
        Action   = "events:*"
        Resource = aws_cloudwatch_event_rule.daily_digest.arn
      },
      {
        Sid      = "SES"
        Effect   = "Allow"
        Action   = "ses:*"
        Resource = "*"
      },
      {
        Sid    = "IAMRoles"
        Effect = "Allow"
        Action = [
          "iam:GetRole",
          "iam:CreateRole",
          "iam:DeleteRole",
          "iam:UpdateRole",
          "iam:UpdateAssumeRolePolicy",
          "iam:PassRole",
          "iam:GetRolePolicy",
          "iam:PutRolePolicy",
          "iam:DeleteRolePolicy",
          "iam:AttachRolePolicy",
          "iam:DetachRolePolicy",
          "iam:ListRolePolicies",
          "iam:ListAttachedRolePolicies",
          "iam:ListInstanceProfilesForRole",
          "iam:TagRole",
          "iam:UntagRole"
        ]
        Resource = [
          aws_iam_role.lambda_exec.arn,
          aws_iam_role.github_actions.arn
        ]
      },
      {
        Sid      = "IAMOIDCProvider"
        Effect   = "Allow"
        Action   = "iam:*OpenIDConnectProvider*"
        Resource = local.github_oidc_provider_arn
      },
      {
        Sid      = "CloudWatchLogs"
        Effect   = "Allow"
        Action   = "logs:*"
        Resource = "arn:aws:logs:*:*:log-group:/aws/lambda/${aws_lambda_function.hndigest.function_name}:*"
      }
    ]
  })
}

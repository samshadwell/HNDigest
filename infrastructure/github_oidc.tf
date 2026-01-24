# GitHub Actions OIDC provider
data "aws_caller_identity" "current" {}

resource "aws_iam_openid_connect_provider" "github" {
  count           = var.create_github_oidc_provider ? 1 : 0
  url             = "https://token.actions.githubusercontent.com"
  client_id_list  = ["sts.amazonaws.com"]
  thumbprint_list = ["d89e3bd43d5d909b47a18977aa9d5ce36cee184c"]
}

locals {
  # Construct ARN directly to avoid needing iam:ListOpenIDConnectProviders permission
  github_oidc_provider_arn = var.create_github_oidc_provider ? aws_iam_openid_connect_provider.github[0].arn : "arn:aws:iam::${data.aws_caller_identity.current.account_id}:oidc-provider/token.actions.githubusercontent.com"
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
        Sid    = "Lambda"
        Effect = "Allow"
        Action = "lambda:*"
        Resource = concat(
          [for k, _ in local.environments : aws_lambda_function.hndigest[k].arn],
          [for k, _ in local.environments : aws_lambda_function.hndigest_api[k].arn]
        )
      },
      {
        Sid      = "DynamoDB"
        Effect   = "Allow"
        Action   = "dynamodb:*"
        Resource = [for k, _ in local.environments : aws_dynamodb_table.hndigest[k].arn]
      },
      {
        Sid      = "EventBridge"
        Effect   = "Allow"
        Action   = "events:*"
        Resource = [for k, _ in local.scheduled_environments : aws_cloudwatch_event_rule.daily_digest[k].arn]
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
        Resource = concat(
          [for k, env in local.environments : aws_iam_role.lambda_exec[k].arn],
          [aws_iam_role.github_actions.arn]
        )
      },
      {
        Sid      = "IAMOIDCProvider"
        Effect   = "Allow"
        Action   = "iam:*OpenIDConnectProvider*"
        Resource = local.github_oidc_provider_arn
      },
      {
        Sid    = "CloudWatchLogs"
        Effect = "Allow"
        Action = "logs:*"
        Resource = concat(
          [for k, env in local.environments : "arn:aws:logs:*:*:log-group:/aws/lambda/${aws_lambda_function.hndigest[k].function_name}:*"],
          [for k, env in local.environments : "arn:aws:logs:*:*:log-group:/aws/lambda/${aws_lambda_function.hndigest_api[k].function_name}:*"],
          [for k, env in local.environments : "arn:aws:logs:*:*:log-group:/aws/apigateway/*"]
        )
      },
      {
        Sid      = "CloudWatchLogsList"
        Effect   = "Allow"
        Action   = "logs:DescribeLogGroups"
        Resource = "arn:aws:logs:*:*:log-group:*"
      },
      {
        Sid    = "LandingPageS3"
        Effect = "Allow"
        Action = "s3:*"
        Resource = [
          aws_s3_bucket.landing_page.arn,
          "${aws_s3_bucket.landing_page.arn}/*"
        ]
      },
      {
        Sid      = "LandingPageACM"
        Effect   = "Allow"
        Action   = "acm:*"
        Resource = aws_acm_certificate.landing_page.arn
      },
      {
        Sid    = "LandingPageCloudFront"
        Effect = "Allow"
        Action = "cloudfront:*"
        Resource = [
          aws_cloudfront_distribution.landing_page.arn,
          aws_cloudfront_origin_access_control.landing_page.arn
        ]
      },
      {
        Sid    = "APIGateway"
        Effect = "Allow"
        Action = "apigateway:*"
        Resource = concat(
          [for k, _ in local.environments : aws_apigatewayv2_api.hndigest[k].arn],
          [for k, _ in local.environments : "${aws_apigatewayv2_api.hndigest[k].arn}/*"]
        )
      }
    ]
  })
}

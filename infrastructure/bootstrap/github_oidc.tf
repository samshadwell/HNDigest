data "aws_caller_identity" "current" {}

resource "aws_iam_openid_connect_provider" "github" {
  count          = var.create_github_oidc_provider ? 1 : 0
  url            = "https://token.actions.githubusercontent.com"
  client_id_list = ["sts.amazonaws.com"]
  thumbprint_list = [
    "1c58a3a8518e8759bf075b76b750d4f2df264fcd",
    "6938fd4d98bab03faadb97b34396831e3780aea1",
  ]
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
          "arn:aws:s3:::${var.bucket_name}",
          "arn:aws:s3:::${var.bucket_name}/*"
        ]
      }
    ]
  })
}

# Policy for managing infrastructure resources
# Uses naming-convention patterns since this role can no longer reference
# resources in other state files
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
        Resource = "arn:aws:lambda:${var.aws_region}:${data.aws_caller_identity.current.account_id}:function:${var.project_name}*"
      },
      {
        Sid      = "DynamoDB"
        Effect   = "Allow"
        Action   = "dynamodb:*"
        Resource = "arn:aws:dynamodb:${var.aws_region}:${data.aws_caller_identity.current.account_id}:table/${var.project_name}*"
      },
      {
        Sid      = "EventBridge"
        Effect   = "Allow"
        Action   = "events:*"
        Resource = "arn:aws:events:${var.aws_region}:${data.aws_caller_identity.current.account_id}:rule/${var.project_name}*"
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
        Resource = "arn:aws:iam::${data.aws_caller_identity.current.account_id}:role/${lower(var.project_name)}*"
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
        Resource = [
          "arn:aws:logs:*:${data.aws_caller_identity.current.account_id}:log-group:/aws/lambda/${var.project_name}*",
          "arn:aws:logs:*:${data.aws_caller_identity.current.account_id}:log-group:/aws/apigateway/${var.project_name}*"
        ]
      },
      {
        Sid      = "CloudWatchLogsList"
        Effect   = "Allow"
        Action   = "logs:DescribeLogGroups"
        Resource = "arn:aws:logs:*:*:log-group:*"
      },
      {
        Sid    = "APIGatewayLogDelivery"
        Effect = "Allow"
        Action = [
          "logs:CreateLogDelivery",
          "logs:UpdateLogDelivery",
          "logs:DeleteLogDelivery",
          "logs:GetLogDelivery",
          "logs:ListLogDeliveries"
        ]
        Resource = "*"
      },
      {
        Sid    = "LandingPageS3"
        Effect = "Allow"
        Action = "s3:*"
        Resource = [
          "arn:aws:s3:::hndigest-landing-page*",
          "arn:aws:s3:::hndigest-landing-page*/*"
        ]
      },
      {
        Sid      = "LandingPageACM"
        Effect   = "Allow"
        Action   = "acm:*"
        Resource = "arn:aws:acm:us-east-1:${data.aws_caller_identity.current.account_id}:certificate/*"
      },
      {
        Sid    = "LandingPageCloudFront"
        Effect = "Allow"
        Action = "cloudfront:*"
        Resource = [
          "arn:aws:cloudfront::${data.aws_caller_identity.current.account_id}:distribution/*",
          "arn:aws:cloudfront::${data.aws_caller_identity.current.account_id}:origin-access-control/*",
          "arn:aws:cloudfront::${data.aws_caller_identity.current.account_id}:cache-policy/*",
          "arn:aws:cloudfront::${data.aws_caller_identity.current.account_id}:origin-request-policy/*"
        ]
      },
      {
        Sid      = "APIGateway"
        Effect   = "Allow"
        Action   = "apigateway:*"
        Resource = "arn:aws:apigateway:${var.aws_region}::*"
      },
      {
        Sid      = "SSM"
        Effect   = "Allow"
        Action   = "ssm:*"
        Resource = "arn:aws:ssm:${var.aws_region}:${data.aws_caller_identity.current.account_id}:parameter/${var.project_name}/*"
      },
      {
        Sid      = "SSMDescribe"
        Effect   = "Allow"
        Action   = "ssm:DescribeParameters"
        Resource = "*"
      },
      {
        Sid      = "SNS"
        Effect   = "Allow"
        Action   = "sns:*"
        Resource = "arn:aws:sns:${var.aws_region}:${data.aws_caller_identity.current.account_id}:${lower(var.project_name)}*"
      },
      {
        Sid      = "CloudWatch"
        Effect   = "Allow"
        Action   = "cloudwatch:*"
        Resource = "arn:aws:cloudwatch:${var.aws_region}:${data.aws_caller_identity.current.account_id}:alarm:${lower(var.project_name)}*"
      },
      {
        Sid      = "SQS"
        Effect   = "Allow"
        Action   = "sqs:*"
        Resource = "arn:aws:sqs:${var.aws_region}:${data.aws_caller_identity.current.account_id}:${lower(var.project_name)}*"
      },
      {
        Sid    = "KMS"
        Effect = "Allow"
        Action = [
          "kms:ListAliases",
          "kms:DescribeKey"
        ]
        Resource = "*"
      }
    ]
  })
}

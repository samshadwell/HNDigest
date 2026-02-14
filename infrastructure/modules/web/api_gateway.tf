# API Gateway HTTP API (v2) for subscription management endpoints
# Note: CloudFront routes /api/* requests to this API Gateway (see landing_page.tf)

resource "aws_apigatewayv2_api" "hndigest" {
  name          = "${var.project_name}${var.name_suffix}-api"
  protocol_type = "HTTP"
}

# Default stage with auto-deploy
resource "aws_apigatewayv2_stage" "default" {
  api_id      = aws_apigatewayv2_api.hndigest.id
  name        = "$default"
  auto_deploy = true

  default_route_settings {
    throttling_burst_limit = 50
    throttling_rate_limit  = 5
  }

  access_log_settings {
    destination_arn = aws_cloudwatch_log_group.api_gateway.arn
    format = jsonencode({
      requestId      = "$context.requestId"
      ip             = "$context.identity.sourceIp"
      requestTime    = "$context.requestTime"
      httpMethod     = "$context.httpMethod"
      routeKey       = "$context.routeKey"
      status         = "$context.status"
      responseLength = "$context.responseLength"
      errorMessage   = "$context.error.message"
    })
  }
}

# CloudWatch log group for API Gateway access logs
resource "aws_cloudwatch_log_group" "api_gateway" {
  name              = "/aws/apigateway/${var.project_name}${var.name_suffix}-api"
  retention_in_days = 14
}

# Lambda integration for API
resource "aws_apigatewayv2_integration" "lambda" {
  api_id                 = aws_apigatewayv2_api.hndigest.id
  integration_type       = "AWS_PROXY"
  integration_uri        = aws_lambda_function.hndigest_api.invoke_arn
  payload_format_version = "2.0"
}

# Route for subscribe endpoint
resource "aws_apigatewayv2_route" "subscribe_post" {
  api_id    = aws_apigatewayv2_api.hndigest.id
  route_key = "POST /api/subscribe"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

# Route for verify endpoint
resource "aws_apigatewayv2_route" "verify_get" {
  api_id    = aws_apigatewayv2_api.hndigest.id
  route_key = "GET /api/verify"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

# Routes for unsubscribe endpoints
resource "aws_apigatewayv2_route" "unsubscribe_get" {
  api_id    = aws_apigatewayv2_api.hndigest.id
  route_key = "GET /api/unsubscribe"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

resource "aws_apigatewayv2_route" "unsubscribe_post" {
  api_id    = aws_apigatewayv2_api.hndigest.id
  route_key = "POST /api/unsubscribe"
  target    = "integrations/${aws_apigatewayv2_integration.lambda.id}"
}

# Permission for API Gateway to invoke Lambda
resource "aws_lambda_permission" "api_gateway" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.hndigest_api.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_api.hndigest.execution_arn}/*/*"
}

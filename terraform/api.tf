resource "aws_apigatewayv2_api" "backend" {
  name                         = "unveil-api-gateway"
  protocol_type                = "HTTP"
  disable_execute_api_endpoint = true
}

resource "aws_apigatewayv2_domain_name" "backend" {
  domain_name = aws_acm_certificate.api.domain_name

  domain_name_configuration {
    certificate_arn = aws_acm_certificate.api.arn
    endpoint_type   = "REGIONAL"
    security_policy = "TLS_1_2"
  }
}

resource "aws_apigatewayv2_api_mapping" "backend" {
  api_id      = aws_apigatewayv2_api.backend.id
  domain_name = aws_apigatewayv2_domain_name.backend.id
  stage       = aws_apigatewayv2_stage.alpha.id
}

resource "aws_api_gateway_account" "backend" {
  // TODO
  cloudwatch_role_arn = "arn:aws:iam::506436569174:role/apigateway-logs"
}

resource "aws_apigatewayv2_vpc_link" "private" {
  name = "unveil-api-gateway-vpc-link"

  security_group_ids = [aws_security_group.unreasonable.id]
  subnet_ids         = [aws_subnet.backend_a.id, aws_subnet.backend_b.id]
}

resource "aws_apigatewayv2_integration" "backend" {
  api_id = aws_apigatewayv2_api.backend.id

  integration_type   = "HTTP_PROXY"
  integration_uri    = aws_lb_listener.api.arn
  integration_method = "ANY"

  request_parameters = {
    "overwrite:path" = "$request.path"
  }

  connection_type = "VPC_LINK"
  connection_id   = aws_apigatewayv2_vpc_link.private.id
}

resource "aws_apigatewayv2_stage" "alpha" {
  api_id = aws_apigatewayv2_api.backend.id

  name = "valpha"

  auto_deploy = true

  access_log_settings {
    destination_arn = aws_cloudwatch_log_group.api_gateway.arn
    format          = <<FORMAT
{ "requestId":"$context.requestId", "ip": "$context.identity.sourceIp", "requestTime":"$context.requestTime", "httpMethod":"$context.httpMethod","routeKey":"$context.routeKey", "status":"$context.status","protocol":"$context.protocol", "responseLength":"$context.responseLength" }
    FORMAT
  }
}

resource "aws_apigatewayv2_route" "default" {
  api_id    = aws_apigatewayv2_api.backend.id
  route_key = "$default"
  target    = "integrations/${aws_apigatewayv2_integration.backend.id}"
}

resource "aws_cloudwatch_log_group" "api_gateway" {
  name = "unveil-apigateway"
}

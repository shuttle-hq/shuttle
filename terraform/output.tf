output "container_repository_url" {
  value = aws_ecr_repository.backend.repository_url
}

output "api_url" {
  value = aws_apigatewayv2_stage.alpha.invoke_url
}

output "user_content_host" {
  value = aws_lb.user.dns_name
}

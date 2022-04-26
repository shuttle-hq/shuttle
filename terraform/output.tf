output "api_url" {
  value       = aws_apigatewayv2_domain_name.backend.id
  description = "URL to connect to the api"
}

output "api_content_host" {
  value = aws_lb.api.dns_name
}

output "user_content_host" {
  value = aws_lb.user.dns_name
}

output "initial_user_key" {
  value       = random_string.initial_key.result
  description = "Key given to the initial shuttle user"
}

output "api_url" {
  value       = aws_apigatewayv2_domain_name.backend.id
  description = "URL to connect to the api"
}

output "api_name_servers" {
  value       = aws_route53_zone.api.name_servers
  description = "Name servers (NS) for api zone"
}

output "db_name_servers" {
  value       = aws_route53_zone.db.name_servers
  description = "Name servers (NS) for pg zone"
}

output "user_name_servers" {
  value       = aws_route53_zone.user.name_servers
  description = "Name servers (NS) for proxy zone"
}

output "api_content_host" {
  value       = aws_lb.api.dns_name
  description = "URL for api load balancer"
}

output "user_content_host" {
  value       = aws_lb.user.dns_name
  description = "URL for user proxy load balancer"
}

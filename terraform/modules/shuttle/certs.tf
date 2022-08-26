resource "aws_route53_zone" "user" {
  name = var.proxy_fqdn
}

resource "aws_acm_certificate" "user" {
  domain_name = var.proxy_fqdn

  subject_alternative_names = [
    "*.${var.proxy_fqdn}"
  ]

  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_route53_record" "user" {
  for_each = {
    for dvo in aws_acm_certificate.user.domain_validation_options : dvo.domain_name => {
      name    = dvo.resource_record_name
      record  = dvo.resource_record_value
      type    = dvo.resource_record_type
      zone_id = aws_route53_zone.user.zone_id
    }
  }

  allow_overwrite = true
  name            = each.value.name
  records         = [each.value.record]
  ttl             = 60
  type            = each.value.type
  zone_id         = each.value.zone_id
}

resource "aws_route53_record" "user_alias" {
  zone_id = aws_route53_zone.user.zone_id
  name    = "*.${var.proxy_fqdn}"
  type    = "A"

  alias {
    name                   = aws_lb.user.dns_name
    zone_id                = aws_lb.user.zone_id
    evaluate_target_health = true
  }
}

resource "aws_acm_certificate_validation" "user" {
  certificate_arn         = aws_acm_certificate.user.arn
  validation_record_fqdns = [for record in aws_route53_record.user : record.fqdn]
}

resource "aws_route53_zone" "api" {
  name = var.api_fqdn
}

resource "aws_acm_certificate" "api" {
  domain_name = var.api_fqdn

  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_route53_record" "api" {
  for_each = {
    for dvo in aws_acm_certificate.api.domain_validation_options : dvo.domain_name => {
      name    = dvo.resource_record_name
      record  = dvo.resource_record_value
      type    = dvo.resource_record_type
      zone_id = aws_route53_zone.api.zone_id
    }
  }

  allow_overwrite = true
  name            = each.value.name
  records         = [each.value.record]
  ttl             = 60
  type            = each.value.type
  zone_id         = each.value.zone_id
}

resource "aws_route53_record" "api_alias" {
  zone_id = aws_route53_zone.api.zone_id
  name    = aws_apigatewayv2_domain_name.backend.domain_name
  type    = "A"

  alias {
    name                   = aws_apigatewayv2_domain_name.backend.domain_name_configuration[0].target_domain_name
    zone_id                = aws_apigatewayv2_domain_name.backend.domain_name_configuration[0].hosted_zone_id
    evaluate_target_health = true
  }
}

resource "aws_acm_certificate_validation" "api" {
  certificate_arn         = aws_acm_certificate.api.arn
  validation_record_fqdns = [for record in aws_route53_record.api : record.fqdn]
}

resource "aws_route53_zone" "db" {
  name = var.db_fqdn
}

resource "aws_acm_certificate" "db" {
  domain_name = var.db_fqdn

  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_route53_record" "db" {
  for_each = {
    for dvo in aws_acm_certificate.db.domain_validation_options : dvo.domain_name => {
      name    = dvo.resource_record_name
      record  = dvo.resource_record_value
      type    = dvo.resource_record_type
      zone_id = aws_route53_zone.db.zone_id
    }
  }

  allow_overwrite = true
  name            = each.value.name
  records         = [each.value.record]
  ttl             = 60
  type            = each.value.type
  zone_id         = each.value.zone_id
}

resource "aws_route53_record" "db_alias" {
  zone_id = aws_route53_zone.db.zone_id
  name    = ""
  type    = "A"

  alias {
    name                   = aws_lb.db.dns_name
    zone_id                = aws_lb.db.zone_id
    evaluate_target_health = true
  }
}

resource "aws_acm_certificate_validation" "db" {
  certificate_arn         = aws_acm_certificate.db.arn
  validation_record_fqdns = [for record in aws_route53_record.db : record.fqdn]
}

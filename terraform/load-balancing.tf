resource "aws_lb" "api" {
  name = "shuttle"

  internal = false

  load_balancer_type = "application"

  security_groups = [aws_security_group.unreasonable.id]
  subnets         = [aws_subnet.backend_a.id, aws_subnet.backend_b.id]

  access_logs {
    bucket  = aws_s3_bucket.logs.bucket
    prefix  = "shuttle-lb"
    enabled = true
  }
}

resource "aws_lb" "db" {
  name = "db"

  internal = false

  load_balancer_type = "network"

  //security_groups = [aws_security_group.unreasonable.id]
  subnets = [aws_subnet.backend_a.id, aws_subnet.backend_b.id]

  access_logs {
    bucket  = aws_s3_bucket.logs.bucket
    prefix  = "db-lb"
    enabled = true
  }
}

resource "aws_lb_target_group" "api" {
  name = "shuttle-lb-tg-http"

  health_check {
    enabled = true
    path    = "/status"
    port    = var.api_container_port
  }

  port = var.api_container_port

  protocol = "HTTP"

  vpc_id = aws_vpc.backend.id

  target_type = "instance"
}

resource "aws_lb_listener" "api" {
  load_balancer_arn = aws_lb.api.arn

  port = "80"

  protocol = "HTTP"

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.api.arn
  }
}

resource "aws_lb_listener" "postgres" {
  load_balancer_arn = aws_lb.db.arn

  port = "5432"

  protocol = "TCP"

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.postgres.arn
  }
}


resource "aws_lb" "user" {
  name = "shuttleapp"

  internal = false

  load_balancer_type = "application"

  security_groups = [aws_security_group.unreasonable.id]
  subnets         = [aws_subnet.backend_a.id, aws_subnet.backend_b.id]

  access_logs {
    bucket  = aws_s3_bucket.logs.bucket
    prefix  = "shuttle-user-lb"
    enabled = true
  }
}

resource "aws_lb_listener" "user" {
  load_balancer_arn = aws_lb.user.arn

  port = "80"

  protocol = "HTTP"

  default_action {
    type = "redirect"

    redirect {
      status_code = "HTTP_301"
      port        = "443"
      protocol    = "HTTPS"
    }
  }
}

resource "aws_lb_listener" "user_tls" {
  load_balancer_arn = aws_lb.user.arn

  port = "443"

  protocol = "HTTPS"

  ssl_policy      = "ELBSecurityPolicy-2016-08"
  certificate_arn = aws_acm_certificate.user.arn

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.user.arn
  }
}

resource "aws_lb_target_group" "user" {
  name = "shuttle-user-lb-tg-http"

  health_check {
    enabled = true
    path    = "/status"
    port    = var.api_container_port
  }

  port = var.proxy_container_port

  protocol = "HTTP"

  vpc_id = aws_vpc.backend.id

  target_type = "instance"
}

resource "aws_lb_target_group" "postgres" {
  name = "shuttle-db-lb-tg-tcp"

  // TODO: change me
  health_check {
    enabled = true
    path    = "/status"
    port    = var.api_container_port
  }

  port = var.postgres_container_port

  protocol = "TCP"

  vpc_id = aws_vpc.backend.id

  target_type = "instance"
}

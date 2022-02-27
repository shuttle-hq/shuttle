resource "aws_lb" "api" {
  name = "unveil-api-load-balancer"

  internal = true

  load_balancer_type = "application"

  security_groups = [aws_security_group.unreasonable.id]
  subnets = [aws_subnet.backend_a.id, aws_subnet.backend_b.id]

  access_logs {
    bucket = aws_s3_bucket.logs.bucket
    prefix = "unveil-lb"
    enabled = true
  }
}

resource "aws_lb_target_group" "api" {
  name = "unveil-lb-tg-http"

  health_check {
    enabled = true
    path = "/status"
    port = var.api_container_port
  }

  port = var.api_container_port

  protocol = "HTTP"

  vpc_id = aws_vpc.backend.id

  target_type = "ip"
}

resource "aws_lb_listener" "api" {
  load_balancer_arn = aws_lb.api.arn

  port = "80"

  protocol = "HTTP"

  default_action {
    type = "forward"
    target_group_arn = aws_lb_target_group.api.arn
  }
}

resource "aws_lb" "user" {
  name = "unveil-user-load-balancer"

  internal = false

  load_balancer_type = "application"

  security_groups = [aws_security_group.unreasonable.id]
  subnets = [aws_subnet.backend_a.id, aws_subnet.backend_b.id]

  access_logs {
    bucket = aws_s3_bucket.logs.bucket
    prefix = "unveil-user-lb"
    enabled = true
  }
}

resource "aws_lb_listener" "user" {
  load_balancer_arn = aws_lb.user.arn

  port = "80"

  protocol = "HTTP"

  default_action {
    type = "forward"
    target_group_arn = aws_lb_target_group.user.arn
  }
}

resource "aws_lb_target_group" "user" {
  name = "unveil-user-lb-tg-http"

  health_check {
    enabled = true
    path = "/status"
    port = var.api_container_port
  }

  port = var.proxy_container_port

  protocol = "HTTP"

  vpc_id = aws_vpc.backend.id

  target_type = "ip"
}

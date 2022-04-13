resource "aws_network_interface" "backend" {
  subnet_id   = aws_subnet.backend_b.id
}

resource "aws_eip" "backend" {
  vpc = true
  network_interface = aws_network_interface.backend.id
}

resource "aws_network_interface_sg_attachment" "backend" {
  security_group_id = aws_security_group.unreasonable.id
  network_interface_id = aws_network_interface.backend.id
}

resource "aws_iam_instance_profile" "backend" {
  name = "backend-profile"
  role = "BackendAPIRole"
}

resource "aws_lb_target_group_attachment" "api" {
  target_group_arn = aws_lb_target_group.api.arn
  target_id        = aws_instance.backend.id
  port             = var.api_container_port
}

resource "aws_lb_target_group_attachment" "user" {
  target_group_arn = aws_lb_target_group.user.arn
  target_id        = aws_instance.backend.id
  port             = var.proxy_container_port
}

resource "aws_lb_target_group_attachment" "postgres" {
  target_group_arn = aws_lb_target_group.postgres.arn
  target_id        = aws_instance.backend.id
  port             = var.postgres_container_port
}

resource "aws_instance" "backend" {
  ami           = "ami-072db068702487a87"  # unveil-backend-ami-20220313
  instance_type = "c6g.4xlarge"

  monitoring = true

  availability_zone = "eu-west-2b"

  iam_instance_profile = aws_iam_instance_profile.backend.id

  network_interface {
    network_interface_id = aws_network_interface.backend.id
    device_index         = 0
  }
}

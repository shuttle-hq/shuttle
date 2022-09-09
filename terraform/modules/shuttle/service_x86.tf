resource "aws_network_interface" "backend_x86" {
  subnet_id = aws_subnet.backend_b.id
}

resource "aws_eip" "backend_x86" {
  vpc               = true
  network_interface = aws_network_interface.backend_x86.id
}

resource "aws_lb_target_group_attachment" "api" {
  target_group_arn = aws_lb_target_group.api.arn
  target_id        = aws_instance.backend_x86.id
  port             = var.api_container_port
}

resource "aws_lb_target_group_attachment" "user" {
  target_group_arn = aws_lb_target_group.user.arn
  target_id        = aws_instance.backend_x86.id
  port             = var.proxy_container_port
}

resource "aws_lb_target_group_attachment" "postgres" {
  target_group_arn = aws_lb_target_group.postgres.arn
  target_id        = aws_instance.backend_x86.id
  port             = var.postgres_container_port
}

resource "aws_lb_target_group_attachment" "mongodb" {
  target_group_arn = aws_lb_target_group.mongodb.arn
  target_id        = aws_instance.backend_x86.id
  port             = var.mongodb_container_port
}

data "aws_ami" "ubuntu_x86" {
  most_recent = true

  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd/ubuntu-focal-20.04-amd64-server-20220511"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }

  owners = ["099720109477"] # Canonical
}

resource "aws_ebs_volume" "backend_x86" {
  availability_zone = "eu-west-2b"
  type              = "gp3"
  size              = 512

  tags = {
    Name = "backend_x86"
  }
}

resource "aws_volume_attachment" "ebs_att" {
  device_name = "/dev/sdh"
  volume_id   = aws_ebs_volume.backend_x86.id
  instance_id = aws_instance.backend_x86.id
}

resource "aws_instance" "backend_x86" {
  ami           = data.aws_ami.ubuntu_x86.id
  instance_type = "c6i.4xlarge"

  monitoring = true

  availability_zone = "eu-west-2b"

  iam_instance_profile = aws_iam_instance_profile.backend.id

  metadata_options {
    http_endpoint = "enabled"
    # Our api runs in a container and therefore has an extra hop limit
    # https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/instancedata-data-retrieval.html#imds-considerations
    http_put_response_hop_limit = 2
    http_tokens                 = "required"
  }

  network_interface {
    network_interface_id = aws_network_interface.backend_x86.id
    device_index         = 0
  }

  root_block_device {
    delete_on_termination = true
    encrypted             = false
    volume_size           = 64
    volume_type           = "gp2"
  }

  user_data                   = data.cloudinit_config.backend.rendered
  user_data_replace_on_change = false
}

resource "aws_vpc" "backend" {
  cidr_block = "10.0.0.0/16"

  enable_dns_hostnames = true
}

resource "aws_internet_gateway" "public" {
  vpc_id = aws_vpc.backend.id
}

resource "aws_network_acl" "unreasonable" {
  vpc_id = aws_vpc.backend.id

  egress {
    protocol = "tcp"
    rule_no = 100
    action = "allow"
    cidr_block = "10.0.0.0/16"
    from_port = 0
    to_port = 65535
  }

  ingress {
    protocol = "tcp"
    rule_no = 100
    action = "allow"
    cidr_block = "10.0.0.0/16"
    from_port = 0
    to_port = 65535
  }
}

resource "aws_security_group" "unreasonable" {
  vpc_id      = aws_vpc.backend.id

  ingress {
    from_port        = 0
    to_port          = 0
    protocol         = "-1"
    cidr_blocks      = ["0.0.0.0/0"]
  }

  egress {
    from_port        = 0
    to_port          = 0
    protocol         = "-1"
    cidr_blocks      = ["0.0.0.0/0"]
  }
}

resource "aws_default_route_table" "backend" {
  default_route_table_id = aws_vpc.backend.default_route_table_id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.public.id
  }
}

resource "aws_subnet" "backend_a" {
  vpc_id = aws_vpc.backend.id

  availability_zone = "eu-west-2a"
  cidr_block = "10.0.10.0/24"
}

resource "aws_subnet" "backend_b" {
  vpc_id = aws_vpc.backend.id

  availability_zone = "eu-west-2b"
  cidr_block = "10.0.20.0/24"
}

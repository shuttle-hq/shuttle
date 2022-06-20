resource "aws_vpc" "backend" {
  cidr_block = "10.0.0.0/16"

  enable_dns_hostnames = true
}

resource "aws_internet_gateway" "public" {
  vpc_id = aws_vpc.backend.id
}

resource "aws_network_acl_rule" "postgres" {
  network_acl_id = aws_vpc.backend.default_network_acl_id
  rule_number    = 10
  egress         = false
  protocol       = "tcp"
  rule_action    = "allow"
  cidr_block     = "0.0.0.0/0"
  from_port      = 5432
  to_port        = 5432
}

resource "aws_network_acl_rule" "mysql" {
  network_acl_id = aws_vpc.backend.default_network_acl_id
  rule_number    = 11
  egress         = false
  protocol       = "tcp"
  rule_action    = "allow"
  cidr_block     = "0.0.0.0/0"
  from_port      = 3306
  to_port        = 3306
}

resource "aws_default_security_group" "default" {
  vpc_id = aws_vpc.backend.id

  ingress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
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
  cidr_block        = "10.0.10.0/24"
}

resource "aws_subnet" "backend_b" {
  vpc_id = aws_vpc.backend.id

  availability_zone = "eu-west-2b"
  cidr_block        = "10.0.20.0/24"
}

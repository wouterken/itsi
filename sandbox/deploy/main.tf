terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 5.0.0"
    }
  }

  required_version = ">= 0.14.9"
}

provider "aws" {
  region = var.aws_region
}


data "aws_region" "current" {
  name = var.aws_region
}

data "aws_ami" "ubuntu" {
  most_recent = true

  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd-gp3/*ubuntu-noble-24.04-arm64-server-*"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }

  owners = ["099720109477"] # Canonical
}

# Create VPC
# terraform aws create vpc
resource "aws_vpc" "vpc" {
  cidr_block           = "10.0.0.0/16"
  instance_tenancy     = "default"
  enable_dns_hostnames = true
  tags = {
    Name = "${var.environment}-vpc"
  }
}
resource "aws_subnet" "public_subnet_a" {
  vpc_id                  = aws_vpc.vpc.id
  cidr_block              = "10.0.1.0/24"
  availability_zone       = var.subnet_az
  map_public_ip_on_launch = true

  tags = {
    Name = "${var.environment} Public Subnet A"
  }
}

resource "aws_subnet" "private_subnet_a" {
  vpc_id            = aws_vpc.vpc.id
  cidr_block        = "10.0.2.0/24"
  availability_zone = var.subnet_az

  tags = {
    Name = "${var.environment} Private Subnet A"
  }
}

resource "aws_internet_gateway" "ig_a" {
  vpc_id = aws_vpc.vpc.id

  tags = {
    Name = "${var.environment} Internet Gateway A"
  }
}

resource "aws_route_table" "public_rt" {
  vpc_id = aws_vpc.vpc.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.ig_a.id
  }

  route {
    ipv6_cidr_block = "::/0"
    gateway_id      = aws_internet_gateway.ig_a.id
  }

  tags = {
    Name = "${var.environment} Public Route Table"
  }
}

resource "aws_route_table_association" "public_1_rt_a" {
  subnet_id      = aws_subnet.public_subnet_a.id
  route_table_id = aws_route_table.public_rt.id
}

resource "aws_security_group" "web_sg" {
  name   = "HTTP and SSH"
  vpc_id = aws_vpc.vpc.id

  ingress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }


  ingress {
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = -1
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "${var.environment} Web Security Group"
  }
}


resource "aws_instance" "web" {
  ami                  = data.aws_ami.ubuntu.id
  instance_type        = "t4g.micro"
  key_name             = "key-pair-aws-admin"
  iam_instance_profile = aws_iam_instance_profile.web.name

  tags = {
    Name = "web-${var.environment}"
  }

  subnet_id                   = aws_subnet.public_subnet_a.id
  security_groups             = [aws_security_group.web_sg.id]
  associate_public_ip_address = true

  user_data = <<EOF
#!/bin/bash
apt-get update
apt-get install zlib1g-dev libyaml-dev libssl-dev libffi-dev libgmp3-dev libclang-dev build-essential -y  && \
apt-get clean && rm -rf /var/lib/apt/lists/* && apt-get autoremove -y
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
curl https://mise.run | sh
echo "eval \"\$(/root/.local/bin/mise activate bash)\"" >> ~/.bashrc
eval "$(/root/.local/bin/mise activate bash)"
mise use ruby@3.4.2
gem install itsi
EOF

  root_block_device {
    encrypted   = true
    volume_type = "gp3"
    volume_size = 12
    tags = {
      Name = "web-root-block-device-${var.environment}"
    }
  }

  depends_on = [aws_route_table_association.public_1_rt_a]
}

locals {
  role_policy_arns = [
    "arn:aws:iam::aws:policy/service-role/AmazonEC2RoleforSSM",
    "arn:aws:iam::aws:policy/CloudWatchAgentServerPolicy"
  ]
}

resource "aws_iam_instance_profile" "web" {
  name = "EC2-Profile-${var.environment}"
  role = aws_iam_role.web.name
}

resource "aws_iam_role_policy_attachment" "web" {
  count = length(local.role_policy_arns)

  role       = aws_iam_role.web.name
  policy_arn = element(local.role_policy_arns, count.index)
}

resource "aws_iam_role_policy" "web" {
  name = "EC2-Inline-Policy-${var.environment}"
  role = aws_iam_role.web.id
  policy = jsonencode(
    {
      "Version" : "2012-10-17",
      "Statement" : [
        {
          "Effect" : "Allow",
          "Action" : [
            "ssm:GetParameter"
          ],
          "Resource" : "*"
        }
      ]
    }
  )
}

resource "aws_iam_role" "web" {
  name = "EC2-Role-${var.environment}"
  path = "/"

  assume_role_policy = jsonencode(
    {
      "Version" : "2012-10-17",
      "Statement" : [
        {
          "Action" : "sts:AssumeRole",
          "Principal" : {
            "Service" : "ec2.amazonaws.com"
          },
          "Effect" : "Allow"
        }
      ]
    }
  )
}

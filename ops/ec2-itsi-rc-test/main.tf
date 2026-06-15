data "aws_ssm_parameter" "al2023_arm64" {
  name = "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-6.1-arm64"
}

data "aws_availability_zones" "available" {
  state = "available"
}

locals {
  az = data.aws_availability_zones.available.names[0]
}

resource "aws_vpc" "this" {
  cidr_block           = "10.77.0.0/16"
  enable_dns_support   = true
  enable_dns_hostnames = true

  tags = {
    Name = "${var.name_prefix}-vpc"
  }
}

resource "aws_internet_gateway" "this" {
  vpc_id = aws_vpc.this.id

  tags = {
    Name = "${var.name_prefix}-igw"
  }
}

resource "aws_subnet" "public" {
  vpc_id                  = aws_vpc.this.id
  cidr_block              = "10.77.1.0/24"
  availability_zone       = local.az
  map_public_ip_on_launch = true

  tags = {
    Name = "${var.name_prefix}-public"
  }
}

resource "aws_route_table" "public" {
  vpc_id = aws_vpc.this.id

  route {
    cidr_block = "0.0.0.0/0"
    gateway_id = aws_internet_gateway.this.id
  }

  tags = {
    Name = "${var.name_prefix}-public"
  }
}

resource "aws_route_table_association" "public" {
  subnet_id      = aws_subnet.public.id
  route_table_id = aws_route_table.public.id
}

resource "aws_security_group" "instance" {
  name        = "${var.name_prefix}-sg"
  description = "It is safe only for ad hoc RC validation."
  vpc_id      = aws_vpc.this.id

  ingress {
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = [var.ssh_cidr]
  }

  ingress {
    from_port   = 80
    to_port     = 80
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  ingress {
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = ["0.0.0.0/0"]
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = {
    Name = "${var.name_prefix}-sg"
  }
}

resource "aws_iam_role" "ssm" {
  name = "${var.name_prefix}-ssm-role"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRole"
      Effect = "Allow"
      Principal = {
        Service = "ec2.amazonaws.com"
      }
    }]
  })
}

resource "aws_iam_role_policy_attachment" "ssm_core" {
  role       = aws_iam_role.ssm.name
  policy_arn = "arn:aws:iam::aws:policy/AmazonSSMManagedInstanceCore"
}

resource "aws_iam_instance_profile" "ssm" {
  name = "${var.name_prefix}-ssm-profile"
  role = aws_iam_role.ssm.name
}

locals {
  user_data = <<-EOF
    #!/bin/bash
    set -euxo pipefail

    dnf update -y
    dnf install -y ruby rubygems ruby-devel gcc gcc-c++ make git tar gzip

    gem install itsi -v ${var.gem_version} --no-document

    mkdir -p /opt/itsi /etc/itsi /var/lib/itsi-acme

    cat >/opt/itsi/Itsi.rb <<'RUBY'
    domain = ENV.fetch("ITSI_DOMAIN")
    email = ENV.fetch("ITSI_ACME_EMAIL")

    bind "http://0.0.0.0:80"
    bind "https://0.0.0.0:443?cert=acme&domains=#{domain}&acme_email=#{email}"

    run proc { |_env|
      [200, { "content-type" => "text/plain" }, ["itsi ok\n"]]
    }
    RUBY

    cat >/usr/local/bin/itsi-runner <<'SH'
    #!/bin/bash
    set -euo pipefail
    set -a
    source /etc/itsi/itsi.env
    set +a
    exec itsi -C /opt/itsi/Itsi.rb -w 1 serve
    SH
    chmod +x /usr/local/bin/itsi-runner

    cat >/usr/local/bin/itsi-configure-domain <<'SH'
    #!/bin/bash
    set -euo pipefail

    if [ "$#" -lt 3 ]; then
      echo "usage: sudo itsi-configure-domain <domain> <email> <staging|prod>" >&2
      exit 1
    fi

    domain="$1"
    email="$2"
    mode="$3"

    cat >/etc/itsi/itsi.env <<EOF_INNER
    ITSI_DOMAIN="$domain"
    ITSI_ACME_EMAIL="$email"
    ITSI_ACME_CACHE_DIR="/var/lib/itsi-acme/$mode"
    EOF_INNER

    if [ "$mode" = "staging" ]; then
      cat >>/etc/itsi/itsi.env <<'EOF_STAGING'
    ITSI_ACME_DIRECTORY_URL="https://acme-staging-v02.api.letsencrypt.org/directory"
    EOF_STAGING
    elif [ "$mode" != "prod" ]; then
      echo "mode must be staging or prod" >&2
      exit 1
    fi

    mkdir -p "/var/lib/itsi-acme/$mode"
    chmod 700 /var/lib/itsi-acme "/var/lib/itsi-acme/$mode"
    systemctl enable --now itsi.service
    systemctl restart itsi.service
    systemctl --no-pager --full status itsi.service
    SH
    chmod +x /usr/local/bin/itsi-configure-domain

    cat >/etc/systemd/system/itsi.service <<'UNIT'
    [Unit]
    Description=Itsi RC test service
    After=network-online.target
    Wants=network-online.target

    [Service]
    Type=simple
    EnvironmentFile=/etc/itsi/itsi.env
    ExecStart=/usr/local/bin/itsi-runner
    Restart=always
    RestartSec=2

    [Install]
    WantedBy=multi-user.target
    UNIT

    cat >/etc/motd <<'MOTD'
    Itsi RC test host

    Next steps:
      1. Point your test domain at this instance.
      2. Run:
         sudo itsi-configure-domain <domain> <email> staging
      3. Verify:
         curl -I http://<domain>/
         curl -vk --resolve <domain>:443:PUBLIC_IP https://<domain>/
      4. When staging is good, switch to prod:
         sudo itsi-configure-domain <domain> <email> prod
    MOTD
  EOF
}

resource "aws_instance" "this" {
  ami                         = data.aws_ssm_parameter.al2023_arm64.value
  instance_type               = var.instance_type
  subnet_id                   = aws_subnet.public.id
  vpc_security_group_ids      = [aws_security_group.instance.id]
  key_name                    = var.key_name
  associate_public_ip_address = true
  iam_instance_profile        = aws_iam_instance_profile.ssm.name
  user_data                   = local.user_data

  root_block_device {
    volume_size = 12
    volume_type = "gp3"
  }

  tags = {
    Name = "${var.name_prefix}-instance"
  }
}

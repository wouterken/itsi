variable "aws_region" {
  type        = string
  description = "AWS region for the test instance."
  default     = "ap-southeast-2"
}

variable "name_prefix" {
  type        = string
  description = "Prefix for resource names."
  default     = "itsi-rc-test"
}

variable "ssh_cidr" {
  type        = string
  description = "Single operator CIDR allowed to SSH."
}

variable "instance_type" {
  type        = string
  description = "EC2 instance type."
  default     = "t4g.nano"
}

variable "key_name" {
  type        = string
  description = "Existing EC2 key pair name."
  default     = "key-pair-aws-admin"
}

variable "gem_version" {
  type        = string
  description = "Itsi gem version to install."
  default     = "0.2.27.rc1"
}

output "instance_id" {
  value = aws_instance.this.id
}

output "public_ip" {
  value = aws_instance.this.public_ip
}

output "public_dns" {
  value = aws_instance.this.public_dns
}

output "ssh_command" {
  value = "ssh -o StrictHostKeyChecking=no -i ~/.ssh/key-pair-aws-admin.pem ec2-user@${aws_instance.this.public_ip}"
}

output "next_step" {
  value = "After DNS points at the instance, run: sudo itsi-configure-domain <domain> <email> staging"
}

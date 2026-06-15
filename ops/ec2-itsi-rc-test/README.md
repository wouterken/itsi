# EC2 Itsi RC Test

Small disposable EC2 environment for testing prerelease Itsi builds and ACME flows.

Default shape:
- region: `ap-southeast-2`
- instance: `t4g.nano`
- AMI: latest Amazon Linux 2023 arm64
- ingress:
  - `22/tcp` from a single operator IP
  - `80/tcp` from anywhere
  - `443/tcp` from anywhere

The instance bootstraps:
- Ruby and RubyGems
- `itsi` gem pinned to the configured version
- a systemd unit for Itsi
- a helper command:

```bash
sudo itsi-configure-domain example.com you@example.com staging
```

That helper writes `/etc/itsi/itsi.env` in systemd `EnvironmentFile` format and starts:

```bash
itsi -C /opt/itsi/Itsi.rb -w 1 serve
```

To verify the origin certificate directly while a proxy is in front, use:

```bash
curl -vk --resolve example.com:443:PUBLIC_IP https://example.com/
```

After testing:

```bash
terraform destroy -var 'ssh_cidr=YOUR_IP/32'
```

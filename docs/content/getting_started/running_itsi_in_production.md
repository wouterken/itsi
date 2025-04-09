---
title: Running Itsi in Production
type: docs
weight: 3
next: /configuration
---

## Docker
Itsi runs well within any Dockerfile that has support for `Ruby 2.7+` and is able to build the Rust toolchain.
You can try Itsi in a Docker container using the official Itsi Docker image
```bash
docker run -it --rm wouterken/itsi:latest
```

See the source of this Dockerfile inside the [Github repository](https://github.com/wouterken/itsi/blob/main/Dockerfile).

## Signal Handling
Itsi supports common signals such as SIGINT and SIGTERM for graceful termination by deployment scripts or container orchestration systems like
K8s and Docker swarm.

{{< callout type="warn" >}}
If you're planning to use Itsi for a high throughput application in K8s, read [this article](https://github.com/puma/puma/blob/master/docs/kubernetes.md) about running
Puma in Kubernetes. The same advice applies to Itsi.
{{< /callout >}}

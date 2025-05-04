---
title: Getting Started
type: docs
weight: 2
prev: features/
next: getting_started/local_development/
---
{{% steps %}}

### Step 1 - Install Ruby

Install Ruby

[https://www.ruby-lang.org/en/documentation/installation/](https://www.ruby-lang.org/en/documentation/installation/)

### Step 2 - Install Itsi

{{< tabs items="Linux,Mac,Windows,FreeBSD" >}}
  {{< tab >}}
  **Prerequisites**


You'll need at least a C/C++ build environment and `clang` and `curl` (for running `rustup`) installed.

#### For Ubuntu / Debian:
```bash
    apt-get install build-essential libclang-dev curl
```
#### For Fedora / RHEL / Rocky / AlmaLinux:
```bash
    dnf groupinstall "Development Tools"
    dnf install clang curl
```
#### For Arch Linux / Manjaro:
```bash
    pacman -S base-devel clang curl
```
#### For Alpine Linux:
```bash
    apk add build-base clang curl
```

Then use `gem` to install Itsi, or its components based on your Ruby version.

**For Ruby >= 3.0**:
```bash
    gem install itsi
```
*(Installs both `itsi-server` and `itsi-scheduler`)*

**For Ruby 2.7**:
```bash
    gem install itsi-server
```
*(Installs `itsi-server` only; `itsi-scheduler` is not supported on Ruby 2.7)*

> ℹ️ **Itsi (`server` + `scheduler`) requires Ruby >= 3.0**
> Itsi `server` supports Ruby >= 2.7
> You can install components individually:
> `gem install itsi-server`
> `gem install itsi-scheduler` (Ruby >= 3.0 only)

  {{< /tab >}}
  {{< tab >}}
  **Mac**:
  **For Ruby >= 3.0**:
```bash
    gem install itsi
```
*(Installs both `itsi-server` and `itsi-scheduler`)*

**For Ruby 2.7**:
```bash
    gem install itsi-server
```
*(Installs `itsi-server` only; `itsi-scheduler` is not supported on Ruby 2.7)*

> ℹ️ **Itsi (`server` + `scheduler`) requires Ruby >= 3.0**
> Itsi `server` supports Ruby >= 2.7
> You can install components individually:
> `gem install itsi-server`
> `gem install itsi-scheduler` (Ruby >= 3.0 only)
  {{< /tab >}}
  {{< tab >}}**Windows**: Itsi currently doesn't support native Windows builds, but it runs well on [WSL](https://learn.microsoft.com/en-us/windows/wsl/install).

  Follow the linked instructions to install a Linux distribution like Ubuntu or Debian, and then follow the instructions in the Linux tab.
  {{< /tab >}}
  {{< tab >}}
  **FreeBSD**

On FreeBSD you'll need to install a few build tools manually:
```bash
  pkg install gmake cmake curl llvm
```

Then install Itsi with GNU make to avoid build errors:
```bash
MAKE=gmake gem install itsi
```

 *(Installs both `itsi-server` and `itsi-scheduler`)*

**For Ruby 2.7**:
```bash
MAKE=gmake gem install itsi-server
```
*(Installs `itsi-server` only; `itsi-scheduler` is not supported on Ruby 2.7)*

  {{< /tab >}}

{{< /tabs >}}

### Step 3 - Learn More

Great! You now have Itsi installed. Go to one of the following pages to learn how to use it:

{{< cards >}}
  {{< card link="./local_development" title="Local Development" icon="star" >}}
  {{< card link="../options" title="Options" icon="adjustments" >}}
  {{< card link="../middleware" title="Middleware" icon="cog" >}}
  {{< card link="https://github.com/wouterken/itsi" title="Source Code" icon="github" >}}
{{< /cards >}}

{{% /steps %}}

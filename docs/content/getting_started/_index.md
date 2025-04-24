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

{{< tabs items="Linux,Mac,Windows" >}}
  {{< tab >}}
  **Prerequisites**

  You'll need at least `build-essential` and `libclang-dev` installed to build Itsi on Linux.
  E.g.
  ```bash
  apt-get install build-essential libclang-dev
  ```

  Then use `gem` to install Itsi, or its components based on your Ruby version.

  **For Ruby >= 3.1**:
  ```bash
  gem install itsi
  ```
  *(Installs both `itsi-server` and `itsi-scheduler`)*

  **For Ruby 2.7 – 3.0**:
  ```bash
  gem install itsi-server
  ```
  *(Installs `itsi-server` only; `itsi-scheduler` is not supported on Ruby < 3.1)*

  {{< callout type="info" >}}
  Itsi (**server + scheduler**) requires **Ruby >= 3.1**.

  Itsi **server** supports **Ruby >= 2.7**.

  If you wish to use either the scheduler or server independently:
  - `gem install itsi-server`
  - `gem install itsi-scheduler` (Ruby >= 3.1 only)

  {{< /callout >}}

  {{< /tab >}}
  {{< tab >}}
  **Mac**:
  **For Ruby >= 3.1**:
  ```bash
  gem install itsi
  ```

  **For Ruby 2.7 – 3.0**:
  ```bash
  gem install itsi-server
  ```

  {{< callout type="info" >}}
  Itsi (**server + scheduler**) requires **Ruby >= 3.1**.

  Itsi **server** supports **Ruby >= 2.7**.

  You can install components individually:
  - `gem install itsi-server`
  - `gem install itsi-scheduler` (Ruby >= 3.1 only)

  ⚠️ Scheduler is not compatible with Ruby 3.0.
  {{< /callout >}}

  {{< /tab >}}
  {{< tab >}}**Windows**: Itsi currently doesn't support native Windows builds, but it runs well on [WSL](https://learn.microsoft.com/en-us/windows/wsl/install).

  Follow the linked instructions to install a Linux distribution like Ubuntu or Debian, and then follow the instructions in the Linux tab.
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

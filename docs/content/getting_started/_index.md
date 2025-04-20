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

  Then use `gem` to install the Itsi package. This will in turn install both the
  `itsi-server` gem, and the `itsi-scheduler` gem.


  ```bash
  gem install itsi
  ```

  {{< callout type="info" >}}
  If you wish to use either the scheduler or server independently, these can be installed individually
  by running `gem install itsi-server` or `gem install itsi-scheduler`.
  {{< /callout >}}

  {{< /tab >}}
  {{< tab >}}
  **Mac**:
  ```bash
  gem install itsi
  ```
  {{< callout type="info" >}}
  If you wish to use either the scheduler or server independently, these can be installed individually
  by running `gem install itsi-server` or `gem install itsi-scheduler`.
  {{< /callout >}}

  {{< /tab >}}
  {{< tab >}}**Windows**: Itsi currently doesn't support native Windows builds, but it runs well on [https://learn.microsoft.com/en-us/windows/wsl/install](WSL).

  Follow the linked instructions to Install a linux distribution like Ubuntu or Debian and then follow the instructions in the Linux tab.
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

---
title: Local Development
type: docs
weight: 3
---

{{< callout>}}
  Itsi provides several optional niceties to enhance your local development experience.
This document is non-essential reading, but worth covering if you're trying Itsi in earnest and want a substantially better
local development experience!
  {{< /callout >}}

## Ruby LSP Add-on
Itsi's [RubyLSP](https://shopify.github.io/ruby-lsp/) add-on allows you to see the full documentation of all of Itsi's [`options`](/options) and [`middleware`](/middleware) directly
inside your editor. It also gives you easy to use auto-completion and snippets for lightning fast changes to `Itsi.rb` configuration files.
You don't need to install the RubyLSP add-on to use Itsi, if both Itsi and RubyLSP are installed and activated in the same project, RubyLSP will automatically
discover and load the addon.

<img src="/ruby-lsp.png" alt="asd" width="700px" style="display: block; margin-left: auto; margin-right: auto;">

## Live Config Reloading
Just add `auto_reload_config!` to your `Itsi.rb` configuration file and Itsi will automatically hot reload its config with every change you make.
Concerned about errors? Itsi will validate your config first before it tries to apply it. If there are errors, Itsi will provide details logs and safely continue with the existing config.

### File Watcher
You can have Itsi watch other files on the file-system and trigger automatic actions in response.
Use the `watch(glob, commands)` method to specify files or directories to watch, and command to execute with each change.
E.g.
```ruby
watch "**.js", [%w[npm run build]]
watch "**.md", [%w[rake docs:build]]
```

## Shell Completions
Itsi can also help you install shell completions, which are useful if you find yourself using the `itsi` executable a lot and forgetting the commands.
Just add the bottom to your `~/.bashrc` or `~/.zshrc` file:

```bash
eval "$(itsi --install-completions)"
```

## Targeted Logging
* Having trouble configuring a specific middleware layer, but debug logs are too verbose? You can change the log-level for a specific middleware layer,
while leaving all other layers at the current global level.
E.g.

```bash
# auth_api_key middleware will log debug messages
# everything else will stick to the INFO level.
ITSI_LOG=info,auth_api_key=debug itsi
```

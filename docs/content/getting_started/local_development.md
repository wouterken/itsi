---
title: Local Development
type: docs
prev: getting_started
weight: 3
---

{{< callout>}}
This document is not required reading, but it can significantly improve your local development experience with Itsi.
{{< /callout >}}

## Ruby LSP Add-on
Itsi's [RubyLSP](https://shopify.github.io/ruby-lsp/) add-on allows you to see the full documentation of all of Itsi's [`options`](/options) and [`middleware`](/middleware) directly
inside your editor. It also gives you easy-to-use auto-completion and snippets for lightning fast changes to `Itsi.rb` configuration files.
You don't need to install the RubyLSP add-on to use Itsi, if both Itsi and RubyLSP are installed and activated in the same project, RubyLSP will automatically
discover and load the addon.

<img src="/ruby-lsp.png" alt="asd" width="700px" style="display: block; margin-left: auto; margin-right: auto;">

## Live Config Reloading
Add `auto_reload_config!` to your `Itsi.rb` configuration file and Itsi will automatically hot reload its config with every change you make.
Concerned about errors? Itsi will validate your config first before it tries to apply it. If there are errors, Itsi will provide detailed logs and safely continue with the existing config.

### File Watcher
You can have Itsi watch other files on the file-system and trigger automatic actions in response.
Use the `watch(glob, commands)` method to specify files or directories to watch, and command to execute with each change.
E.g.
```ruby
watch "**.js", [%w[npm run build]]
watch "**.md", [%w[rake docs:build]]
```

## Print Routes
Itsi comes with a built-in command to see all the routes that are configured in your application. To use it, simply run the following command:
```bash
itsi routes
```

E.g.
```bash
────────────────────────────────────────────────────────────────────────────
Route:      /app/users/(?<id>[^/]+
Conditions: (none)
Middleware: • log_requests(before: I am th..., after: [{reque...)
            • compress
            • cors(*, GET POST PUT DELETE)
            • app /Users/pico/Development/itsi/gems/server/lib/itsi/server/typed_handlers.rb:9
────────────────────────────────────────────────────────────────────────────
Route:      /app/users/?
Conditions: (none)
Middleware: • log_requests(before: I am th..., after: [{reque...)
            • compress
            • cors(*, GET POST PUT DELETE)
            • app /Users/pico/Development/itsi/gems/server/lib/itsi/server/rack_interface.rb:15

```
## Test Config
Itsi allows you to validate your configuration without having to run the application.

```bash
itsi test
```

You can optionally provide an explicit config file path using
```bash
itsi test -C /path/to/Itsi.rb
```



## Shell Completions
Itsi can also help you install shell completions, which are useful if you find yourself using the `itsi` executable a lot and forgetting the commands.
Add the following line to the bottom of your ~/.bashrc or ~/.zshrc file:

```bash
eval "$(itsi --install-completions)"
```

## macOS Fork Safety Considerations

On macOS, using fork() in multithreaded applications can lead to crashes due to the Objective-C runtime’s behavior. This is particularly relevant when working with tools like Itsi that may utilize fork() under the hood.

### Understanding the Issue

Apple’s Objective-C runtime is not fork-safe in multithreaded environments. When a process that uses Objective-C APIs forks, the child process may crash if it interacts with the Objective-C runtime before calling exec(). This behavior is by design to prevent potential deadlocks and inconsistent states. ￼ ￼

Common symptoms include errors like:
```
objc[51435]: +[__NSCFConstantString initialize] may have been in progress in another thread when fork() was called.
We cannot safely call it or ignore it in the fork() child process. Crashing instead.
```

### Workarounds:
To mitigate these issues, consider the following environment variables:
*	`OBJC_DISABLE_INITIALIZE_FORK_SAFETY=YES`: Disables the Objective-C runtime’s fork safety checks. Use with caution, as it may mask underlying issues . ￼ ￼
*	`PGGSSENCMODE=disable`: Disables GSSAPI encryption in PostgreSQL, which can cause issues in forked processes .

Itsi includes a mechanism to automatically re-execute itself with the necessary environment variables set when running on macOS, effectively performing the above workarounds for you.

If you prefer to manage these settings yourself, you can disable this behavior by setting the `ITSI_DISABLE_AUTO_DISABLE_DARWIN_FORK_SAFETY_WARNINGS` environment variable:

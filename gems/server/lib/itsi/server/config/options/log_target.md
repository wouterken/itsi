---
title: Log Target
url: /options/log_target
---

Configures the target for logging. The default value is `:stdout`.

## Configuration
```ruby {filename=Itsi.rb}
log_target :stdout
```

```ruby {filename=Itsi.rb}
# Log to a file instead
log_target "my_app.log"
```

```ruby {filename=Itsi.rb}
# Log to both a file and the standard output (filename determined by ITSI_LOG_FILE)
log_target :both
```

## Options
| Option   | Description                                                                 |
|----------|-----------------------------------------------------------------------------|
| stdout     | Logs are sent to the standard output (console). This is the default option. |
| [filename] | Logs are written to a specified file.                                       |
| both       | Logs are sent to both the standard output the default log file.            |


## Environment Variables
You can also control the log target using environment variables.
If both are set, the value in the configuration file takes precedence.

| Variable   | Description                                                                 |
|------------|-----------------------------------------------------------------------------|
| ITSI_LOG_TARGET | Specifies the log target. Possible values are stdout, filename, or both.    |
| ITSI_LOG_FILE | The name of the log file used by itsi. Default is `itsi-app.log`.    |

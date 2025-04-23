---
title: Signals
type: docs
weight: 5
next: /configuration

---

Itsi responds to several Unix signals for process control. These signals are used to gracefully shut down, reload configuration, adjust worker pools, or emit internal lifecycle events.


## Signal Types

| Signal     | Behavior                                                                 |
|------------|--------------------------------------------------------------------------|
| `SIGINT`   | Triggers a graceful shutdown. If received twice in quick succession, triggers a forceful shutdown. |
| `SIGTERM`  | Triggers a graceful shutdown.                                            |
| `SIGUSR1`  | Triggers a hot restart (Sockets remain open, while configuration is reloaded). |
| `SIGUSR2`  | Triggers a diagnostic info dump. Causes all child processes to print detailed diagnostic information.                           |
| `SIGHUP`   | Triggers a reload of configuration. If using preloading mode, this is equivalent to a restart. Otherwise this will cause a phased restart.                           |
| `SIGTTIN`  | Increases the number of worker processes.            |
| `SIGTTOU`  | Decreases the number of worker processes.            |

---

## CLI Convenience Commands

In addition to sending the above signals through native controls, you can also use the `itsi` executable as a convenient shortcut for sending signals.
These will work so long as the command is run inside the same directory from which Itsi was started (as this is the directory within which the `pid` file is located).

- `itsi stop`: Sends `SIGINT` to initiate a graceful shutdown.
- `itsi restart`: Sends `SIGHUP` to trigger a reload of configuration.
- `itsi reload`: Sends `SIGUSR1` to trigger a hot restart.
- `itsi status`: Sends `SIGUSR2` to trigger a diagnostic info dump.
- `itsi add_worker`: Sends `SIGTTIN` to increase the number of worker processes.
- `itsi remove_worker`: Sends `SIGTTOU` to decrease the number of worker processes.

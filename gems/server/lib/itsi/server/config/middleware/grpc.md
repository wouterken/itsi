---
title: gRPC
url: /middleware/grpc
---

The **gRPC** middleware lets you expose one or more Ruby gRPC service handlers directly in Itsi.
This allows you to use Itsi's efficient asynchronous server to serve requests and gain enhanced performance and asynchronous capabilities.

Under the covers it:

1. Serves a full HTTP/2 gRPC endpoint (with binary frames, trailers, `HTTP/2` compression, and reflection).
2. Provides a **JSON‑over‑HTTP** gateway for each unary or streaming method—so you can POST JSON and receive JSON arrays without a gRPC client.
3. Automatically enables `gRPC` reflection (so client like `evans`, `grpcurl` or `Postman` can discover your service endpoints without needing access to raw `.proto` files).
4. Supports optional per‑call compression (`none`, `deflate`, `gzip`) and a non‑blocking execution mode.

---

## Usage

```ruby
# Define (or require) your service implementation:
# Mount it in your Itsi.rb:
grpc EchoServiceImpl.new,
     nonblocking: false do         # prepend response with grpc‑encoding:gzip
  # any additional middleware can nest here, e.g.:
  response_headers additions: { "X-Service" => ["Echo"] }
end
```

## Options

| Option        | Type    | Default | Description                                                   |
|---------------|---------|---------|---------------------------------------------------------------|
| `*handlers`  | Object  | –       | One or more gRPC service implementations  |
| `nonblocking`   | Boolean | false   | Run handler in fiber/thread‑pool (nonblocking mode). Only effective if using [hybrid mode](/options/scheduler_threads), otherwise the gRPC handler will adopt the server global concurrency mode (threads or fibers)         |
| `reflection`   | Boolean | true   | Determines whether to serve reflection endpoints. (Useful for clients to auto-discover services)         |
| `&block`        | Proc    | –       | **Optional** You can add additional middleware inside an optional block, to apply to all gRPC requests (e.g. rate limiters, deny lists, max body etc. etc.).                |

Reflection is served only over HTTP/2; JSON‑over‑HTTP works with HTTP/1.1.



## Walkthrough
The below is a simple walkthrough of using Itsi and the [Ruby gRPC](https://grpc.io/docs/languages/ruby/basics/) library to expose a simple "Echo" service in Itsi,  complete with both native gRPC/HTTP2 and a JSON‑over‑HTTP gateway.

{{% steps %}}

### Step 1 — Define your gRPC contract

Create a file `echo.proto` with:
```proto
syntax = "proto3";
package echo;

service EchoService {
  rpc Echo(EchoRequest) returns (EchoResponse);
}

message EchoRequest {
  string message = 1;
}

message EchoResponse {
  string message = 1;
}
```

### Step 2 — Generate Ruby stubs

Install the Ruby gRPC tools and run:
```bash
gem install grpc-tools
grpc_tools_ruby_protoc -I . --ruby_out=./ --grpc_out=./ echo.proto
```
This produces `echo_services_pb.rb` and supporting files.

### Step 3 — Implement the service

Create `echo_service_impl.rb`:
```ruby {filename=echo_service_impl.rb}
require_relative 'echo_services_pb'

class EchoServiceImpl < Echo::EchoService::Service
  # Unary RPC implementation
  def echo(req, _unused_call)
    Echo::EchoResponse.new(message: req.message)
  end
end
```

### Step 4 — Mount in Itsi

In your `Itsi.rb` at the project root, add:

```ruby {filename=Itsi.rb}
require_relative 'echo_service_impl'

bind "https://localhost:3000"
grpc EchoServiceImpl.new,
      nonblocking: false,
      compression: 'gzip' do
  # Nested middleware still works:
  response_headers additions: { 'X-Service' => ['Echo'] }
end
```
### Step 5 — Start the server
```bash
itsi serve
```

### Step 6 — Test with a gRPC client

E.g. using [Evans](https://github.com/ktr0731/evans)

```bash
evans --host localhost --port 3000 repl

  ______
 |  ____|
 | |__    __   __   __ _   _ __    ___
 |  __|   \ \ / /  / _. | | '_ \  / __|
 | |____   \ V /  | (_| | | | | | \__ \
 |______|   \_/    \__,_| |_| |_| |___/

 more expressive universal gRPC client


echo.EchoService@127.0.0.1:3000> call Echo
message (TYPE_STRING) => Hello
{
  "message": "Hello"
}

echo.EchoService@127.0.0.1:3000>
```

### Step 7 — Try the JSON gateway

You can also POST plain JSON (works over HTTP/1.1 or HTTP/2):
```bash
curl \
  -H 'Content-Type: application/json' \
  -H "Content-Type: application/json" \
  -X POST http://localhost:3000/echo.EchoService/Echo \
  -d '{"message":"world"}'

{"message":"world"}

```

{{% /steps %}}


## JSON gateway
Itsi exposes gRPC services via a secondary JSON gateways for use with simple HTTP clients.
Once mounted, each service‐method is *also* reachable via HTTP/2 and via simple JSON POSTs:

```bash
# Unary RPC (Echo)
curl -X POST http://0.0.0.0:3000/EchoService/Echo \
     -H "Content-Type: application/json" \
     -d '{"message":"hello"}'
# → 200 OK, body: {"message":"hello"}

# Server streaming RPC (e.g. "Numbers"): POST an array, receive JSON array:
curl -X POST http://0.0.0.0:3000/NumberService/Stream \
     -H "Content-Type: application/json" \
     -d '[{"n":1},{"n":2},{"n":3}]'
# → 200 OK, body: [{"n":1},{"n":2},{"n":3}]
```

Under the hood, the same framing machinery is used—you just get plain JSON arrays instead of gRPC frames.

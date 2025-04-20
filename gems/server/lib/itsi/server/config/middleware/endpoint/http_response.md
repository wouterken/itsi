---
title: HTTP Response
url: /middleware/http_response
---

Within the body of an [endpoint](/middleware/endpoint), you can access the `#response` object on the incoming request for more fine-grained control over the response body.


{{< callout >}}
Itsi allows you to efficiently keep long-running responses open, and write to these asynchronously.
Just remember to close them eventually...

{{< /callout >}}


E.g.

```ruby {filename=Itsi.rb}

get "/" do |req|
  resp = req.response
  resp << "Stream some content"

  # Eventually... (This does not have to occur within the body of this method.)
  resp.close
end
```


| Method           | Description                                                                 |
|------------------|-----------------------------------------------------------------------------|
| `#<<`            | Appends content to the response body (allows you to stream content).                                       |
| `#send_and_close`| Sends a single response chunk and closes the connection.                              |
| `status=`        | Sets the HTTP status code for the response.                                |
| `add_header`     | Adds a header to the response.                                             |
| `accept`         | Retrieves the accepted content types from the request.                    |
| `close`          | Closes the response stream.                                               |
| `json?`          | Checks if the response content type is JSON.                              |
| `html?`          | Checks if the response content type is HTML.                              |

---
title: OOB GC
url: /options/oob_gc
---

The **OOB GC Response Threshold** sets the threshold at which the Ruby GC should be triggered during periods where there is a gap in queued requests.

## Configuration File

### Examples

```ruby {filename="Itsi.rb"}
oob_gc_response_threshold 1000 # Trigger GC every 1000 pauses/gaps
```


{{< callout >}}
Settings this too aggressively can seriously impact performance. It's recommended to start with a relatively high value and then adjust based on your application's specific needs and performance characteristics.
{{< /callout >}}

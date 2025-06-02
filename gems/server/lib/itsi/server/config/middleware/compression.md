---
title: Compression
url: /middleware/compression
---

The compression middleware allows you to configure compression settings for your application.
You can enable several different compression algorithms, and choose to selectively apply these based on the request path, content type, mime-type, and size. You can also choose whether or not to compress streams, and adjust the compression level.


## Top level compression
```ruby {filename=Itsi.rb}
  compress \
    min_size: 1024 # 1KiB,
    algorithms: %w[zstd gzip deflate br],
    compress_streams: true,
    mime_types: %w[all],
    level: "fastest"
```

## Compression within a location block
```ruby {filename=Itsi.rb}

  location "/images" do
    compress \
      min_size: 1024 # 1KiB,
      algorithms: %w[zstd gzip deflate br],
      mime_types: %w[image],
      level: "fastest"

    static_assets: \
      ...
  end
```

## Parameters

| Parameter | Description |
| --- | --- |
| `min_size` | The minimum size of the response body in bytes before compression is applied. Default is `1024` (1KiB). |
| `algorithms` | An array of compression algorithms to use. Supports any combination  of `zstd`, `gzip`, `deflate`, `br`. |
| `compress_streams` | Whether or not to compress streams. Default is `true`. |
| `mime_types` | An array of mime-type groups/classes as string to compress. Default is `["all"]`.<br/>Available options are `all`, `text`, `image`, `audio`, `video`, `application`, `font`. <br/>You can also match arbitrary mime-types, by using an `other` object instead `{ "other" => "other/type" }` |
| `level` | The compression level to use. Default is `fastest`. Can be any of `fastest`, `best`, `balanced` and  `precise` |

<br/>

# Pre-compressed `static_assets`
Itsi also supports serving pre-compressed static assets directly from the file-system.
This is configured inside the `static_assets` middleware.
Go to the [static_assets](/middleware/static_assets) middleware for more information.

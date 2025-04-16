require_relative "../helpers/test_helper"
require "tmpdir"

class TestStaticAssets < Minitest::Test
    # Test serving an existing file via GET.
  def test_serving_existing_file
    Dir.mktmpdir do |dir|
      file_path = File.join(dir, "test.txt")
      File.write(file_path, "Hello world")
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: "fallthrough",
            headers: { "Cache-Control" => "max-age=3600" },
            allowed_extensions: ["txt"]

          get("/test.txt") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/test.txt")
        assert_equal "200", res.code, "Expected 200 OK for an existing file"
        assert_equal "Hello world", res.body
        assert_match /max-age=3600/, res["Cache-Control"]
      end
    end
  end

  # Test HEAD request returns headers with no body.
  def test_head_request
    Dir.mktmpdir do |dir|
      file_path = File.join(dir, "test.txt")
      File.write(file_path, "Hello world")
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: "fallthrough",
            allowed_extensions: ["txt"]

          get("/test.txt") { |r| r.ok "fallback" }
        end
      ) do
        res = head("/test.txt")
        assert_equal "200", res.code, "Expected 200 OK for HEAD request"
        assert(res.body.nil? || res.body.empty?, "Expected empty body for HEAD request")
      end
    end
  end

  # Test that non-GET/HEAD requests are not handled by static asset middleware.
  def test_non_get_head_request_passes_through
    Dir.mktmpdir do |dir|
      File.write(File.join(dir, "test.txt"), "Hello world")
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: "fallthrough",
            allowed_extensions: ["txt"]

          endpoint { |r| r.ok "fallback" }
        end
      ) do
        res = post("/test.txt","")
        # Since POST is not handled by static assets, expect the fallback.
        assert_equal "200", res.code, "Expected fallback response for non-GET/HEAD request"
        assert_equal "fallback", res.body
      end
    end
  end

  # Test try_html_extension: If "test.html" exists, a request to "/test" returns its content.
  def test_try_html_extension
    Dir.mktmpdir do |dir|
      File.write(File.join(dir, "test.html"), "HTML content")
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: "fallthrough",
            try_html_extension: true,
            allowed_extensions: ["html"]

          get("/test") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/test")
        assert_equal "200", res.code, "Expected 200 OK with try_html_extension"
        assert_equal "HTML content", res.body
      end
    end
  end

  # Test a Range request for partial content.
  def test_range_request
    Dir.mktmpdir do |dir|
      File.write(File.join(dir, "test.txt"), "Hello world")
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: "fallthrough",
            allowed_extensions: ["txt"]

          get("/test.txt") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/test.txt", { "Range" => "bytes=0-4" })
        # Expecting 206 Partial Content with "Hello" as the body.
        assert_equal "206", res.code, "Expected 206 Partial Content for valid Range request"
        assert_equal "Hello", res.body
      end
    end
  end

  # Test that a non-existent file passes through to the fallback handler.
  def test_file_not_found_passes_through
    Dir.mktmpdir do |dir|
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: "fallthrough",
            allowed_extensions: ["txt"]

          get("/nonexistent") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/nonexistent")
        assert_equal "200", res.code, "Expected fallback response for non-existent file"
        assert_equal "fallback", res.body
      end
    end
  end
  def test_relative_path_true
    Dir.mktmpdir do |dir|
      # Create file "test.txt" in root_dir.
      dir = FileUtils.mkdir_p(File.join(dir, "foo", "bar", "baz")).first
      file_path = File.join(dir, "test.txt")
      File.write(file_path, "relative true")
      server(
        itsi_rb: lambda do
          location "foo/bar/baz" do
            static_assets \
              root_dir: "#{dir}",
              not_found_behavior: "fallthrough",
              allowed_extensions: ["txt"],
              relative_path: true

            get("/foo/bar/baz/test.txt") { |r| r.ok "fallback" }
          end
        end
      ) do
        res = get_resp("/foo/bar/baz/test.txt")
        assert_equal "200", res.code, "Expected 200 OK with relative_path: true"
        assert_equal "relative true", res.body
      end
    end
  end

  def test_relative_path_false
    Dir.mktmpdir do |dir|
      # Create file "test.txt" in root_dir.
      child_dir = FileUtils.mkdir_p(File.join(dir, "foo", "bar", "baz")).first
      file_path = File.join(child_dir, "test.txt")

      File.write(file_path, "relative false")
      server(
        itsi_rb: lambda do

          location "/foo/bar/baz/" do
            static_assets \
              root_dir: "#{dir}",
              not_found_behavior: "fallthrough",
              allowed_extensions: ["txt"],
              relative_path: false

            get("/foo/bar/baz/test.txt") { |r| r.ok "fallback" }
          end
        end
      ) do
        res = get_resp("/foo/bar/baz/test.txt")

        assert_equal "200", res.code, "Expected 200 OK with relative_path: false"
        assert_equal "relative false", res.body
      end
    end
  end

  # 2. Allowed Extensions Test
  def test_file_not_in_allowed_extensions
    Dir.mktmpdir do |dir|
      file_path = File.join(dir, "file.xyz")
      File.write(file_path, "not allowed")
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: "fallthrough",
            allowed_extensions: ["txt"]
          get("/file.xyz") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/file.xyz")
        assert_equal "200", res.code, "Expected fallback when file extension is not allowed"
        assert_equal "fallback", res.body
      end
    end
  end

  def test_hidden_files_not_served_when_disabled
    Dir.mktmpdir do |dir|
      file_path = File.join(dir, ".secret.txt")
      File.write(file_path, "hidden content")
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: "fallthrough"
          endpoint { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/.secret.txt")

        assert_equal "200", res.code, "Expected fallback when hidden files are disabled"
        assert_equal "fallback", res.body
      end
    end
  end

  def test_hidden_files_served_when_enabled
    Dir.mktmpdir do |dir|
      file_path = File.join(dir, ".secret.txt")
      File.write(file_path, "hidden content")
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: "fallthrough",
            allowed_extensions: ["txt"],
            serve_hidden_files: true
          get("/.secret.txt") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/.secret.txt")
        assert_equal "200", res.code, "Expected 200 OK when hidden files are enabled"
        assert_equal "hidden content", res.body
      end
    end
  end

  # 4. Auto Index Tests
  def test_auto_index_json
    Dir.mktmpdir do |dir|
      File.write(File.join(dir, "file1.txt"), "content1")
      File.write(File.join(dir, "file2.txt"), "content2")
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: "fallthrough",
            auto_index: true,
            allowed_extensions: []
          get("/") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/", { "Accept" => "application/json" })

        assert_equal "200", res.code, "Expected 200 OK for JSON auto index"
        assert_includes res.body, "[", "Expected JSON listing in auto index"
      end
    end
  end

  def test_auto_index_html
    Dir.mktmpdir do |dir|
      File.write(File.join(dir, "file1.txt"), "content1")
      File.write(File.join(dir, "file2.txt"), "content2")
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: "fallthrough",
            auto_index: true,
            allowed_extensions: []
          get("/") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/", { "Accept" => "text/html" })
        assert_equal "200", res.code, "Expected 200 OK for HTML auto index"
        assert_includes res.body.downcase, "<html", "Expected HTML structure in auto index"
      end
    end
  end

  # 5. Not Found Behavior Tests
  def test_not_found_behavior_index
    Dir.mktmpdir do |dir|
      # Create an index file to be used as fallback.
      File.write(File.join(dir, "index.html"), "SPA Index")
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: { index: "index.html" },
            allowed_extensions: ["html"]
          get("/nonexistent") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/nonexistent")
        assert_equal "200", res.code, "Expected index to be served for index not found behaviour"
        assert_equal "SPA Index", res.body
      end
    end
  end

  def test_not_found_behavior_redirect
    Dir.mktmpdir do |dir|
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: { redirect: { to: "https://example.com", type: "permanent" } }
          get("/nonexistent") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/nonexistent")
        assert_equal "308", res.code, "Expected 308 for redirect not found behaviour"
        assert_equal "https://example.com", res["Location"]
      end
    end
  end

  def test_not_found_behavior_error_builtin
    Dir.mktmpdir do |dir|
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: {error: "not_found"}
          get("/nonexistent") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/nonexistent")
        assert_equal "404", res.code, "Expected 404 for built-in not_found behaviour"
      end
    end
  end

  def test_not_found_behavior_custom_error
    Dir.mktmpdir do |dir|
      server(
        itsi_rb: lambda do
          static_assets \
            root_dir: "#{dir}",
            not_found_behavior: {
              error: {
                code: 404,
                plaintext: { inline: "Not Found" },
                html: { inline: "<h1>Not Found</h1>" },
                default: "plaintext"
              },
            }
          get("/nonexistent") { |r| r.ok "fallback" }
        end
      ) do
        res = get_resp("/nonexistent")
        assert_equal "404", res.code, "Expected 404 for custom not_found behaviour"
        assert_equal "Not Found", res.body
      end
    end
  end
end

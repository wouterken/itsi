require_relative "../helpers/test_helper"
require "json"

class TestCsp < Minitest::Test
  def test_header_is_added
    server(
      itsi_rb: lambda do
        csp \
          policy: {
            default_src: ["'self'"],
            script_src: ["example.com"]
          }
        get("/") { |r| r.ok "hello" }
      end
    ) do
      res = get_resp("/")
      header = res["Content-Security-Policy"]
      assert header.include?("default-src 'self'")
      assert header.include?("script-src example.com")
    end
  end

  def test_reports_get_logged
    Tempfile.create("csp-log") do |f|
      server(
        itsi_rb: lambda do
          csp \
            policy: {
              default_src: ["'none'"],
              report_uri: ["/csp-report"]
            },
            reporting_enabled: true,
            report_file: f.path,
            report_endpoint: "/csp-report",
            flush_interval: 0.1

          get("/") { |r| r.ok "ok" }
        end
      ) do
        report = {
          "csp-report" => {
            "document-uri" => "http://example.com/",
            "referrer" => "",
            "violated-directive" => "script-src",
            "original-policy" => "default-src 'none';",
            "blocked-uri" => "inline"
          }
        }

        post("/csp-report", JSON.dump(report), {
          "Content-Type" => "application/csp-report"
        })

        sleep 0.15

        content = File.read(f.path)
        assert_includes content, "violated-directive"
        assert_includes content, "blocked-uri"
      end
    end
  end
end

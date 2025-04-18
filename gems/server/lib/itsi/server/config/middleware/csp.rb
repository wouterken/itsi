module Itsi
  class Server
    module Config
      class Csp < Middleware
        insert_text <<~SNIPPET
          csp \\
            policy: {
              default_src: ${1:["'self'"]},
              script_src: ${2:["'self'", "cdn.example.com"]},
              style_src: ${3:["'self'"]},
              report_uri: ${4:["/csp-report"]}
            },
            reporting_enabled: ${5|true,false|},
            report_file: "${6:csp_reports.json}",
            report_endpoint: "${7:/csp-report}",
            flush_interval: ${8:5.0}
        SNIPPET

        detail "Adds Content-Security-Policy headers and collects violation reports."

        CspPolicy = TypedStruct.new do
          {
            default_src: Array(Type(String)).default([]),
            script_src: Array(Type(String)).default([]),
            style_src: Array(Type(String)).default([]),
            report_uri: Array(Type(String)).default([])
          }
        end

        schema do
          {
            policy: (Type(CspPolicy) & Required()).default({default_src: [], script_src: [], style_src: [], report_uri: []}),
            reporting_enabled: Bool().default(false),
            report_file: Type(String),
            report_endpoint: Type(String).default("/csp-report"),
            flush_interval: Type(Float).default(5.0)
          }
        end


      end
    end
  end
end

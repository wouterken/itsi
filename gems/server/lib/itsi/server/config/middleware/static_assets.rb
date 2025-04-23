module Itsi
  class Server
    module Config
      class StaticAssets < Middleware

        insert_text <<~SNIPPET
          static_assets \\
            root_dir: "${1|./,/var/www,|}",
            not_found_behavior: ${2|"fallthrough",{index: "index.html"},{error: "not_found"}|},
            auto_index: ${3|true,false|},
            try_html_extension: ${4|true,false|},
            max_file_size_in_memory: ${5|1048576,2097152|},
            max_files_in_memory: ${6|100,500,1000|},
            file_check_interval: ${7|1,60,120|},
            headers: { ${8|,"Cache-Control" => "max-age=3600",|} },
            allowed_extensions: ${9|%w[html css js png jpg],[]|},
            relative_path: ${10|true,false|},
            serve_hidden_files: ${11|true,false|}
        SNIPPET

        detail "Serves static files from a designated directory with options for auto indexing, in-memory caching, and custom header support. Supports relative path rewriting and file range requests."

        ErrorResponse = TypedStruct.new do
          {
            error: Type(ErrorResponseDef) & Required()
          }
        end

        IndexResponse = TypedStruct.new do
          {
            index: Type(String) & Required()
          }
        end

        RedirectResponse = TypedStruct.new do
          {
            redirect: Type(Redirect::Redirect) & Required()
          }
        end

        schema do
          {
            root_dir: (Type(String) & Required()).default("./"),
            not_found_behavior: Or(
              Enum(["fallthrough", "index", "redirect", "internal_server_error"]),
              Type(IndexResponse),
              Type(RedirectResponse),
              Type(ErrorResponse)
            ).default({error: "not_found"}),
            allowed_extensions: (Array(Type(String)) & Required()).default([]),
            auto_index: Bool().default(false),
            try_html_extension: Bool().default(true),
            max_file_size_in_memory: Type(Integer).default(1048576),
            max_files_in_memory: Type(Integer).default(100),
            file_check_interval: Type(Integer).default(1),
            headers: Hash(Type(String), Type(String)).default({}),
            relative_path: Bool().default(true),
            serve_hidden_files: Bool().default(false)
          }
        end

        def build!
          root_dir = @params[:root_dir] || "."

          if !File.exist?(root_dir)
            raise "Warning: static_assets root_dir '#{root_dir}' does not exist!"
          elsif !File.directory?(root_dir)
            raise "Warning: static_assets root_dir '#{root_dir}' is not a directory!"
          end

          @params[:relative_path] = true unless @params.key?(:relative_path)
          @params[:allowed_extensions] ||= []

          if @params[:try_html_extension] && @params[:allowed_extensions].include?("html")
            @params[:allowed_extensions] << ""
          end

          if @params[:allowed_extensions].any? && @params[:auto_index]
            @params[:allowed_extensions] |= ["html"]
            @params[:allowed_extensions] |= [""]
          end

          @params[:base_path] = "^(?<base_path>#{location.paths_from_parent.gsub(/\.\*\)$/,")")}).*$"
          params = @params

          location.middleware[:static_assets] = params
        end
      end
    end
  end
end

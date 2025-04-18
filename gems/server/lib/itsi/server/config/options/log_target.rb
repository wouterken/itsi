module Itsi
  class Server
    module Config
      class LogTarget < Option

        insert_text <<~SNIPPET
        log_target ${1|:stdout,:both,"./filename.log"|}
        SNIPPET

        detail "Specifies the target for logging. The default value is stdout."

        schema do
          (Type(String) & Or(Enum(%w[stdout file]), Type(String))).default('stdout')
        end

      end
    end
  end
end

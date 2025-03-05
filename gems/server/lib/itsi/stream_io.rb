module Itsi
  class StreamIO
    attr :response
    def initialize(response)
      @response = response
    end

    def read
      response.recv_frame
    end

    def write(string)
      response.send_frame(string)
    end

    def <<(string)
      response.send_frame(string)
    end

    def flush
      # No-op
    end

    def close
      close_read
      close_write
    end

    def close_read
      response.close_read
    end

    def close_write
      response.close_write
    end

  end
end

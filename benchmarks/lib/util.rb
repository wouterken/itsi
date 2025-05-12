
def wait_for_port(port, timeout: 5)
  Timeout.timeout(timeout) do
    loop do
      TCPSocket.new('127.0.0.1', port).close
      break
    rescue Errno::ECONNREFUSED, Errno::EHOSTUNREACH
      sleep 0.1
    end
  end
rescue Timeout::Error
  raise "Unable to connect to localhost:#{port}"
end


def free_port
  server = TCPServer.new("0.0.0.0", 0)
  port = server.addr[1]
  server.close
  port
end

def parse_wrk_output(output)
  metrics = {}

  if output =~ /Requests\/sec:\s+([\d.]+)/
    metrics[:requests_per_sec] = $1.to_f
  end

  if output =~ /Transfer\/sec:\s+([\d.]+)([KMG]?B)/
    metrics[:transfer_per_sec] = {
      value: $1.to_f,
      unit: $2
    }
  end

  if output =~ /Latency\s+([\d.]+)([a-z]+)/
    metrics[:latency_avg] = {
      value: $1.to_f,
      unit: $2
    }
  end

  metrics
end

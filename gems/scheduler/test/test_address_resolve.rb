# frozen_string_literal: true
require 'debug'


class TestAddressResolve < Minitest::Test
  include Itsi::Scheduler::TestHelper

  def test_addess_resolve
    results = []

    with_scheduler do |_scheduler|
      Fiber.schedule do
        results << Addrinfo.getaddrinfo("www.ruby-lang.org", 80, nil, :STREAM)
      end
      Fiber.schedule do
        results << Addrinfo.getaddrinfo("www.google.com", 80, nil, :STREAM)
      end
    end

    assert  results.all?{|results| results.find(&:ipv4?) }
    assert results.all?{|results| results.find(&:ipv6?) }
  end
end

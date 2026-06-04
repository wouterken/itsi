# frozen_string_literal: true

require "ipaddr"

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

    assert_equal 2, results.length
    assert results.all?(&:any?)
    results.flatten.each do |addrinfo|
      assert_instance_of Addrinfo, addrinfo
      assert IPAddr.new(addrinfo.ip_address)
    end
  end
end

require 'minitest/autorun'
require 'ipaddr'

class TestCidrToRegex < Minitest::Test
  include Itsi::Server::CidrToRegex

  def test_simple_cidr
    regex = cidr_to_regex("192.168.1.0/24")
    assert_match regex, "192.168.1.0"
    assert_match regex, "192.168.1.255"
    refute_match regex, "192.168.2.0"
  end

  def test_class_b_cidr
    regex = cidr_to_regex("10.1.0.0/16")
    assert_match regex, "10.1.0.1"
    assert_match regex, "10.1.255.255"
    refute_match regex, "10.2.0.0"
  end

  def test_class_a_cidr
    regex = cidr_to_regex("10.0.0.0/8")
    assert_match regex, "10.255.255.255"
    refute_match regex, "11.0.0.0"
  end

  def test_tiny_range
    regex = cidr_to_regex("127.0.0.0/30")
    assert_match regex, "127.0.0.0"
    assert_match regex, "127.0.0.3"
    refute_match regex, "127.0.0.4"
  end

  def test_single_ip
    regex = cidr_to_regex("8.8.8.8/32")
    assert_match regex, "8.8.8.8"
    refute_match regex, "8.8.8.9"
  end

  def test_edge_case_lower_bound
    regex = cidr_to_regex("0.0.0.0/8")
    assert_match regex, "0.0.0.0"
    assert_match regex, "0.255.255.255"
    refute_match regex, "1.0.0.0"
  end
end

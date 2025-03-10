# frozen_string_literal: true


class TestItsiScheduler < Minitest::Test
  def test_that_it_has_a_version_number
    refute_nil ::Itsi::Scheduler::VERSION
  end
end

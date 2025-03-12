# frozen_string_literal: true

require "active_record"

class TestActiveRecordFiberScheduler < Minitest::Test
  include Itsi::Scheduler::TestHelper

  # Set up an ActiveRecord connection to your PostgreSQL test database.
  # Adjust the connection parameters as needed.
  def setup
    ActiveSupport::IsolatedExecutionState.isolation_level = :fiber
    ActiveRecord::Base.establish_connection(
      adapter: "postgresql",
      database: "fiber_scheduler_test",
      pool: 2, # use a small pool to test contention scenarios
      checkout_timeout: 5
    )
  end

  # Disconnect after each test.
  def teardown
    ActiveRecord::Base.connection_pool.disconnect!
  end

  # Test a basic query execution inside a fiber.
  def test_basic_query
    result = nil

    with_scheduler do |_scheduler|
      Fiber.schedule do
        result = ActiveRecord::Base.connection.select_value("SELECT 1")
      end
    end

    # select_value returns a string from PG adapter so we compare with "1"
    assert_equal "1", result.to_s
  end

  # Test running two queries concurrently in different fibers.
  def test_concurrent_queries
    results = []

    with_scheduler do |_scheduler|
      Fiber.schedule do
        results << ActiveRecord::Base.connection.select_value("SELECT 1")
      end

      Fiber.schedule do
        results << ActiveRecord::Base.connection.select_value("SELECT 2")
      end
    end

    # Ensure that both queries have executed and returned the expected values.
    results = results.map(&:to_s)
    assert_includes results, "1"
    assert_includes results, "2"
  end

  # Test a query that involves a short delay using PostgreSQL's pg_sleep.
  def test_query_with_delay
    result = nil

    with_scheduler do |_scheduler|
      Fiber.schedule do
        # Introduce a 0.1 second delay.
        ActiveRecord::Base.connection.execute("SELECT pg_sleep(0.1)")
        result = ActiveRecord::Base.connection.select_value("SELECT 3")
      end
    end

    assert_equal "3", result.to_s
  end

  # Test connection pool exhaustion by limiting the pool to one connection.
  # Two fibers will attempt to get a connection concurrently.
  def test_connection_pool_exhaustion
    # Re-establish connection with a pool size of 1.
    ActiveSupport::IsolatedExecutionState.isolation_level = :fiber
    ActiveRecord::Base.establish_connection(
      adapter: "postgresql",
      host: "localhost",
      database: "fiber_scheduler_test",
      pool: 1,
      checkout_timeout: 0.25
    )
    # ActiveRecord::Base.connection_pool.disconnect!

    results = []

    with_scheduler do |_scheduler|
      Fiber.schedule do
        ActiveRecord::Base.connection_pool.with_connection(prevent_permanent_checkout: true) do
          results << ActiveRecord::Base.connection.select_value("SELECT 1")
        end
      end

      Fiber.schedule do
        ActiveRecord::Base.connection_pool.with_connection(prevent_permanent_checkout: true) do
          results << ActiveRecord::Base.connection.select_value("SELECT 2")
        end
      end
    end

    # Takes #{checkout_timeout} seconds between last
    results = results.map(&:to_s)
    assert_includes results, "1"
    assert_includes results, "2"
  end

  # Test that after a Fiber finishes its work, its connection is automatically released.
  def test_fiber_connection_release_after_completion
    # Use the scheduler to run a fiber that checks out a connection and does a simple query.

    with_scheduler do |_scheduler|
      Fiber.schedule do
        # This fiber checks out a connection to run a query.
        ActiveRecord::Base.connection.select_value("SELECT 1")
        # The fiber ends here.
      end
    end

    # After the scheduler finishes, the fiber should have completed and released its connection.
    # Now we attempt to checkout a connection manually. If the previous fiber's connection
    # was not released, this would either time out or raise an error.
    connection = ActiveRecord::Base.connection_pool.checkout
    assert connection, "Expected to obtain a connection after fiber completion"
    ActiveRecord::Base.connection_pool.checkin(connection)
  end

  # Test that a transaction works correctly when run inside a fiber.
  # A temporary table is created, a record inserted and then queried before the transaction is rolled back.
  def test_transaction_fiber
    result = nil

    with_scheduler do |_scheduler|
      Fiber.schedule do
        ActiveRecord::Base.transaction do
          # Create a temporary table for testing.
          ActiveRecord::Base.connection.execute(<<~SQL)
            CREATE TEMP TABLE IF NOT EXISTS test_table (
              id serial PRIMARY KEY,
              name text
            )
          SQL

          # Insert a record.
          ActiveRecord::Base.connection.execute("INSERT INTO test_table (name) VALUES ('Alice')")
          # Query the inserted record.
          result = ActiveRecord::Base.connection.select_value("SELECT name FROM test_table LIMIT 1")
          # Roll back the transaction to avoid leaving test data.
          raise ActiveRecord::Rollback
        end
      end
    end

    assert_equal "Alice", result.to_s
  end
end

# Implementation of the Echo service
require 'grpc'
require 'google/protobuf'
require 'google/type/money_pb'
require 'google/protobuf/timestamp_pb'
require 'google/protobuf/any_pb'

descriptor_data = "\n\necho.proto\x12\x04\x65\x63ho\x1a\x17google/type/money.proto\x1a\x1fgoogle/protobuf/timestamp.proto\x1a\x19google/protobuf/any.proto\"\x1e\n\x0b\x45\x63hoRequest\x12\x0f\n\x07message\x18\x01 \x01(\t\".\n\x0c\x45\x63hoResponse\x12\x0f\n\x07message\x18\x01 \x01(\t\x12\r\n\x05\x63ount\x18\x02 \x01(\x05\"\xa3\x01\n\x0ePaymentRequest\x12\x13\n\x0b\x63ustomer_id\x18\x01 \x01(\t\x12\"\n\x06\x61mount\x18\x02 \x01(\x0b\x32\x12.google.type.Money\x12\x30\n\x0cpayment_time\x18\x03 \x01(\x0b\x32\x1a.google.protobuf.Timestamp\x12&\n\x08metadata\x18\x04 \x01(\x0b\x32\x14.google.protobuf.Any\"\xae\x01\n\x0fPaymentResponse\x12\x16\n\x0etransaction_id\x18\x01 \x01(\t\x12\x30\n\x0cprocessed_at\x18\x02 \x01(\x0b\x32\x1a.google.protobuf.Timestamp\x12#\n\x06status\x18\x03 \x01(\x0e\x32\x13.echo.PaymentStatus\x12\x1a\n\rerror_message\x18\x04 \x01(\tH\x00\x88\x01\x01\x42\x10\n\x0e_error_message*\x82\x01\n\rPaymentStatus\x12\x1e\n\x1aPAYMENT_STATUS_UNSPECIFIED\x10\x00\x12\x1a\n\x16PAYMENT_STATUS_SUCCESS\x10\x01\x12\x19\n\x15PAYMENT_STATUS_FAILED\x10\x02\x12\x1a\n\x16PAYMENT_STATUS_PENDING\x10\x03\x32\xb4\x02\n\x0b\x45\x63hoService\x12/\n\x04\x45\x63ho\x12\x11.echo.EchoRequest\x1a\x12.echo.EchoResponse\"\x00\x12\x37\n\nEchoStream\x12\x11.echo.EchoRequest\x1a\x12.echo.EchoResponse\"\x00\x30\x01\x12\x38\n\x0b\x45\x63hoCollect\x12\x11.echo.EchoRequest\x1a\x12.echo.EchoResponse\"\x00(\x01\x12@\n\x11\x45\x63hoBidirectional\x12\x11.echo.EchoRequest\x1a\x12.echo.EchoResponse\"\x00(\x01\x30\x01\x12?\n\x0eProcessPayment\x12\x14.echo.PaymentRequest\x1a\x15.echo.PaymentResponse\"\x00\x62\x06proto3" # rubocop:disable Layout/LineLength

pool = Google::Protobuf::DescriptorPool.generated_pool
pool.add_serialized_file(descriptor_data)

module Echo
  EchoRequest = ::Google::Protobuf::DescriptorPool.generated_pool.lookup('echo.EchoRequest').msgclass
  EchoResponse = ::Google::Protobuf::DescriptorPool.generated_pool.lookup('echo.EchoResponse').msgclass
  PaymentRequest = ::Google::Protobuf::DescriptorPool.generated_pool.lookup('echo.PaymentRequest').msgclass
  PaymentResponse = ::Google::Protobuf::DescriptorPool.generated_pool.lookup('echo.PaymentResponse').msgclass
  PaymentStatus = ::Google::Protobuf::DescriptorPool.generated_pool.lookup('echo.PaymentStatus').enummodule
end

module Echo
  module EchoService
    class Service # rubocop:disable Style/Documentation
      include ::GRPC::GenericService

      self.marshal_class_method = :encode
      self.unmarshal_class_method = :decode
      self.service_name = 'echo.EchoService'

      # Simple unary method
      rpc :Echo, ::Echo::EchoRequest, ::Echo::EchoResponse
      # Server streaming method
      rpc :EchoStream, ::Echo::EchoRequest, stream(::Echo::EchoResponse)
      # Client streaming method
      rpc :EchoCollect, stream(::Echo::EchoRequest), ::Echo::EchoResponse
      # Bidirectional streaming method
      rpc :EchoBidirectional, stream(::Echo::EchoRequest), stream(::Echo::EchoResponse)
      # Payment processing method
      rpc :ProcessPayment, ::Echo::PaymentRequest, ::Echo::PaymentResponse
    end

    Stub = Service.rpc_stub_class
  end
end

class EchoServiceImpl < Echo::EchoService::Service
  def echo(request, _call)
    @counter ||= 1
    # sleep 0.05
    Echo::EchoResponse.new(message: "Echo: #{request.message}", count: @counter += 1)
  end

  def echo_stream(request, _call)
    # Stream 3 responses back

    3.times do |i|
      response = Echo::EchoResponse.new(
        message: "Echo[#{i}]: #{request.message}",
        count: i + 1
      )
      yield response
      sleep 0.1
    end
  end

  def echo_collect(requests, _call)
    messages = []
    count = 0

    # Collect all messages from the client
    requests.each do |req|
      messages << req.message
      count += 1
    end

    # Return a single response with all messages concatenated
    Echo::EchoResponse.new(
      message: "Collected: #{messages.join(', ')}",
      count: count
    )
  end

  def echo_bidirectional(requests, _call)
    count = 0

    # For each request received, send a response immediately
    thr = Thread.new do
      requests.each do |_|
        count += 1
        sleep 0.1
      end
    end

    Enumerator.new do |e|
      10.times do |_i|
        e <<  Echo::EchoResponse.new(
          message: "Here's a response\n",
          count: count
        )
      end
      thr.join
    end
  end

  def process_payment(request, _call) # rubocop:disable Metrics/MethodLength
    # Create a response with current timestamp
    #
    processed_at = Google::Protobuf::Timestamp.new(seconds: Time.now.to_i)

    # Generate a mock transaction ID
    transaction_id = "txn_#{Time.now.to_i}_#{rand(1000..9999)}"

    # Simulate payment processing
    status = if request.amount.units.positive?
               Echo::PaymentStatus::PAYMENT_STATUS_SUCCESS
             else
               Echo::PaymentStatus::PAYMENT_STATUS_FAILED
             end

    # Create the response
    Echo::PaymentResponse.new(
      transaction_id: transaction_id,
      processed_at: processed_at,
      status: status,
      error_message: status == Echo::PaymentStatus::PAYMENT_STATUS_FAILED ? 'Invalid amount' : nil
    )
  end
end

# frozen_string_literal: true

require "mkmf"
require "rb_sys/mkmf"

create_rust_makefile("itsi/server/itsi_server") do |r|
  r.extra_rustflags = ["-C target-cpu=native"]
end

# frozen_string_literal: true

require 'json'
require 'active_record'
require 'net/http'
require 'debug'


TMP_DB_DIR  = Dir.mktmpdir('io_party_db')
TMP_DB_FILE = File.join(TMP_DB_DIR, 'test.sqlite3')

ActiveRecord::Base.establish_connection(adapter: 'sqlite3', database: TMP_DB_FILE)

ActiveRecord::Schema.define do
  create_table :posts, force: true do |t|
    t.string :name
    t.text   :body
    t.timestamps
  end
end

class Post < ActiveRecord::Base; end

run(
  proc do |_|
    post = Post.find_or_create_by(name: 'Hello World', body: 'I made a change. This is a test post')
    ActiveRecord::Base.connection.execute('SELECT * FROM posts;')
    sleep 0.0001

    queue = Queue.new
    Thread.new do
      sleep 0.0001
      queue.push('done')
    end.join
    queue.pop

    Thread.new do
      sleep 0.0001
    end.join

    post.update(name: 'I made a change. Hello World', body: 'Wow... I think it might be working')
    [200, { 'content-type' => 'text/plain'}, [post.to_json]]
  end
)

at_exit do
  ActiveRecord::Base.connection_pool.disconnect!
  FileUtils.rm_f(TMP_DB_FILE) # remove database file
  begin
    Dir.rmdir(TMP_DB_DIR)
  rescue StandardError
    nil
  end
end

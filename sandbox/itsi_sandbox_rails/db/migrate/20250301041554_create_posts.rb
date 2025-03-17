class CreatePosts < ActiveRecord::Migration[7.2]
  def change
    create_table :posts do |t|
      t.text :name
      t.text :body

      t.timestamps
    end
  end
end

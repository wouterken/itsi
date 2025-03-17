# app/controllers/uploads_controller.rb
class UploadsController < ApplicationController
  # Disable CSRF for this endpoint (for testing only)
  skip_before_action :verify_authenticity_token

  def body
    render plain: params
  end

  def create
    uploaded_file = params[:file]

    if uploaded_file
      metadata = {
        filename: uploaded_file.original_filename,
        content_type: uploaded_file.content_type,
        size: uploaded_file.size,
        # If you want to read the first few bytes (as it comes in)
        head: uploaded_file.read(100)
      }
      # Reset the file pointer if needed:
      uploaded_file.rewind

      render plain: metadata.to_s
    else
      render plain: "No file uploaded", status: 400
    end
  end
end

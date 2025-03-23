class OrganisationsController
  def organisation_serve(request)
    response = request.response
    response.status = 200
    response.add_header('Content-Type', 'text/plain')
    response << 'Serve Organisation!'
    response.close
  end

  def organisation_create(request)
    puts 'Create organisation'
    request.close
  end
end

location '/organisations' do
  controller OrganisationsController.new
  get '/:id', :organisation_serve
  post '/:id', :organisation_create
end

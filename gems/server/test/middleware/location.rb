require_relative "../helpers/test_helper"

class TestNestedLocation < Minitest::Test
  # Tests that a nested block takes precedence over its parent block
  def test_basic_nested_route
    server(
      itsi_rb: lambda do
        # The outer location "/" applies to all routes.
        location "/" do
          # A nested location matching "/foo" with its own GET handler.
          location "/foo" do
            get("/bar") { |r| r.respond("From nested /foo/bar") }
          end
          # This GET handler is only hit if no nested location applies.
          get("/bar") { |r| r.respond("From root /bar") }
        end
      end
    ) do
      res_nested = get_resp("/foo/bar")
      assert_equal "From nested /foo/bar", res_nested.body

      res_root = get_resp("/bar")
      assert_equal "From root /bar", res_root.body
    end
  end

  # Tests deep nesting where dynamic segments are used
  def test_deeply_nested_route
    server(
      itsi_rb: lambda do
        location "/v1" do
          location "/users" do
            # A nested capture for user id.
            location "/:user_id" do
              get("/profile") { |r, params| r.respond("User profile for v1/users/#{params[:user_id]}") }
            end
          end
        end
      end
    ) do
      res = get_resp("/v1/users/123/profile")
      assert_equal "User profile for v1/users/123", res.body
    end
  end

  # Tests that ordering of nested location blocks is honored. Only the first matching block should execute.
  def test_nested_route_ordering
    server(
      itsi_rb: lambda do
        location "/" do
          location "/alpha" do
            # This is the first nested match for "/alpha/beta"
            location "/beta" do
              get("/gamma") { |r| r.respond("Response from first nested /alpha/beta/gamma") }
            end
            # Though this block also matches "/alpha/beta", it should never be reached.
            location "/beta" do
              get("/gamma") { |r| r.respond("Response from second nested /alpha/beta/gamma") }
            end
          end
          # This handler is defined on the outer level and is a fallback.
          get("/alpha/beta/gamma") { |r| r.respond("Response from outer /alpha/beta/gamma") }
        end
      end
    ) do
      res = get_resp("/alpha/beta/gamma")
      # According to the recursive matching rules, the first matching child (/alpha then /beta) is used.
      assert_equal "Response from first nested /alpha/beta/gamma", res.body
    end
  end

  # Tests multiple nested levels under a common outer location.
  def test_multiple_nested_options
    server(
      itsi_rb: lambda do
        location "/api" do
          get("/status") { |r| r.respond("API Status from /api") }
          location "/users" do
            get("/list") { |r| r.respond("User List from /api/users") }
            location "/:user_id" do
              get("/details") { |r| r.respond("User Details from /api/users/:user_id") }
            end
          end
        end
      end
    ) do
      res_status = get_resp("/api/status")
      assert_equal "API Status from /api", res_status.body

      res_list = get_resp("/api/users/list")
      assert_equal "User List from /api/users", res_list.body

      res_details = get_resp("/api/users/42/details")
      assert_equal "User Details from /api/users/:user_id", res_details.body
    end
  end

  def test_dynamic_vs_wildcard_precedence
     server(
       itsi_rb: lambda do
         location "/products" do
           # This nested block should match when a numeric id is provided.
           location "/:id([0-9]+)" do
             get("/details") { |r| r.respond("Product details for numeric id") }
           end
           # Fallback handler for when the numeric match does not occur.
           get("/details") { |r| r.respond("General product details") }
         end
       end
     ) do
       res_dynamic = get_resp("/products/123/details")
       assert_equal "Product details for numeric id", res_dynamic.body

       res_fallback = get_resp("/products/details")
       assert_equal "General product details", res_fallback.body
     end
   end

   # Test deep nesting with multiple dynamic segments (year, month, slug) versus a fallback archive route.
   def test_multiple_dynamic_segments
     server(
       itsi_rb: lambda do
         location "/blog" do
           # Nested dynamic segments for a blog post.
           location "/:year([0-9]{4})" do
             location "/:month([0-9]{1,2})" do
               location "/:slug" do
                 get { |r, params| r.respond("Blog post: #{params[:year]}/#{params[:month]}/#{params[:slug]}") }
               end
             end
           end
           # Fallback route for blog archive.
           get("/archive") { |r| r.respond("Blog archive") }
         end
       end
     ) do
       res_post = get_resp("/blog/2021/9/challenge-post")
       assert_equal "Blog post: 2021/9/challenge-post", res_post.body

       res_archive = get_resp("/blog/archive")
       assert_equal "Blog archive", res_archive.body
     end
   end

   # Test mixed static and dynamic segments. The static route should override the dynamic one for an exact match.
   def test_static_vs_dynamic_precedence
     server(
       itsi_rb: lambda do
         location "/dashboard" do
           # Static route for settings.
           location "/settings" do
             get { |r| r.respond("Dashboard settings") }
           end
           # Dynamic route for other sections.
           location "/:section" do
             get { |r, params| r.respond("Dashboard section: #{params[:section]}") }
           end
         end
       end
     ) do
       res_static = get_resp("/dashboard/settings")
       assert_equal "Dashboard settings", res_static.body

       res_dynamic = get_resp("/dashboard/stats")
       assert_equal "Dashboard section: stats", res_dynamic.body
     end
   end

   # Test overlapping regex-based routing versus a fallback dynamic match.
   def test_overlapping_regex_and_fallback
     server(
       itsi_rb: lambda do
         location "/files" do
           # This nested location will match when the filename ends with .txt.
           location "/:filename([a-z]+\.txt)" do
             get { |r, params| r.respond("Text file: #{params[:filename]}") }
           end
           # Fallback for any file that doesn't match the above regex.
           location "/:filename" do
             get { |r, params| r.respond("Other file: #{params[:filename]}") }
           end
         end
       end
     ) do
       res_txt = get_resp("/files/readme.txt")
       assert_equal "Text file: readme.txt", res_txt.body

       res_other = get_resp("/files/readme.pdf")
       assert_equal "Other file: readme.pdf", res_other.body
     end
   end

   # Test a deeper nested structure that mimics a versioned API with sub-resources.
   def test_complex_nested_structure
     server(
       itsi_rb: lambda do
         location "/api" do
           location "/v1" do
             location "/users" do
               location "/:user_id" do
                 location "/profile" do
                   get("/") { |r, params| r.respond("User Profile for #{r.params[:user_id]}") }
                 end
                 location "/orders" do
                   get("/") { |r, params| r.respond("User Orders for #{r.params[:user_id]}") }
                 end
               end
             end
           end
         end
       end
     ) do
       res_profile = get_resp("/api/v1/users/42/profile")
       assert_equal "User Profile for 42", res_profile.body

       res_orders = get_resp("/api/v1/users/42/orders")
       assert_equal "User Orders for 42", res_orders.body
     end
   end
end

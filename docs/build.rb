require 'json'

def replace_nans(obj)
  case obj
  when Array
    obj.map { |e| replace_nans(e) }
  when Hash
    obj.transform_values { |v| replace_nans(v) }
  when Float
    obj.nan? ? 0.0 : obj
  else
    obj
  end
end

system('cd benchmark-dashboard && '\
  'npm run build && '\
  'cp dist/benchmark-dashboard.iife.js ../static/scripts &&'\
  'cp dist/benchmark-dashboard.css ../static/styles')

combined = replace_nans(
  Dir['../benchmarks/results/*'].map do |pth|
    {
      cpu: File.basename(pth),
      groups: Dir["#{pth}/*"].map do |f|
        {
          group: File.basename(f),
          tests: Dir["#{f}/*"].map do |f|
            {
              test: File.basename(f),
              servers: Dir["#{f}/*.json"].map do |s|
                {
                  server: File.basename(s, '.json'),
                  results: JSON.parse(IO.read(s), allow_nan: true)
                }
              end
            }
          end
        }
      end
    }
  end
)

IO.write(
  './static/results.json',
  combined.to_json
)
IO.write(
  './public/results.json',
  combined.to_json
)

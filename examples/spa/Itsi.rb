# For an SPA you typically want to route missing files back to an index.html file
static_assets root_dir: "./dist", not_found_behavior: {index: "index.html"}

# Rebuild the app each time one of our source files change.
watch "./**/*.jsx", [%w[npm run build]]

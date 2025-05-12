path(["public/image.png", "public/index.html", "dynamic"])

static_files_root "./apps"

requires %i[static ruby]

concurrency_levels([10, 25, 50, 75, 100])

app File.open('apps/static.ru')

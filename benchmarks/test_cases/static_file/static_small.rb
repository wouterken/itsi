path "public"

static_files_root "./apps"

requires %i[static]

app File.open('apps/static.ru')

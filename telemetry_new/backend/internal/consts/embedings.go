package consts
import "embed"

// shared static files from the sveltekit application
// embedding all the static files into the binary
// static files
//go:embed all:frontend_build
var FrontendFiles embed.FS

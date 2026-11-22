// Package route implements static files
package routes

import (
	"hyped/backend_telemetry/internal/consts"
	"io/fs"
	"log"
	"strings"
	"sync"

	"github.com/gofiber/fiber/v3"
	"github.com/gofiber/fiber/v3/middleware/static"
)

type Store struct{
	files fs.FS
}


var (
	instance *Store
	once sync.Once
	staticErr error
)

func InitStatic() error {
	once.Do(func() {
	subFS, err := fs.Sub(consts.FrontendFiles, "frontend_build")
		if err != nil {
			staticErr = err
			log.Panicln("Something very bad happened when packaging the Go Binary")
		}
		instance = &Store{files:subFS}
	})
	return staticErr
}

func StaticFiles(c fiber.Ctx) error {
	subFS := instance.files
	return static.New("", static.Config{
		FS: subFS,
		NotFoundHandler: func(c fiber.Ctx) error {
			path := c.Path()
			if strings.Contains(path, ".") {
				return c.SendStatus(fiber.StatusNotFound)
			}
			html := strings.TrimPrefix(path, "/") + ".html"
			if html == ".html" {
				html = "index.html"
			}
			if f, err := subFS.Open(html); err == nil {
				err := f.Close()
				if err != nil {
					return ErrorErrorError(c)
				}
				if content, readErr := fs.ReadFile(subFS, html); readErr == nil {
					c.Type("html")
					return c.Send(content)
				}
			}
			if content, readErr := fs.ReadFile(subFS, "404.html"); readErr == nil {
				c.Type("html")
				return c.Status(404).Send(content)
			}
			return ErrorErrorError(c)
		},
	},
	)(c)
}

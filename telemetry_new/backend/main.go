package main

import (
	"embed"
	"io/fs"
	"log"
	"strings"

	"hyped/backend_telemetry/internal/auth"
	"hyped/backend_telemetry/internal/database"
	"hyped/backend_telemetry/internal/routes"

	"github.com/gofiber/fiber/v3"
	"github.com/gofiber/fiber/v3/middleware/cors"
	"github.com/gofiber/fiber/v3/middleware/static"
	_ "github.com/lib/pq"
)

//go:embed all:frontend_build
var frontendFiles embed.FS

func main() {
	db, err := database.Connect(database.PsqlConnectionString())
	if err != nil {
		log.Println("Cannot connect to the database!")
	}

	// TESTING ONLY
	log.Println("TEST MODE IS ON")
	user, err := database.CreateUser(db, "example@example.com", "John Testing", "testing")
	if err != nil {
		log.Println("User might have existed already!")
		user, err := database.GetUserByEmail(db, "example@example.com")
		if err == nil {
			err := database.AddUserToRoles(db, user, "admin")
			if err != nil {
				log.Println(err.Error())
			}
		}
	} else {
		if database.AddUserToRoles(db, user, "admin") != nil {
			log.Fatalln("Error while creating test user")
		}
	}
	log.Println("test token: ")
	log.Println(auth.CreateToken("example@example.com"))
	// TESTING ONLY

	app := fiber.New()
	app.Use(cors.New())

	app.Use(cors.New(cors.Config{
		AllowOrigins: []string{"https://localhost:8080"},
		AllowHeaders: []string{"Origin", "Content-Type", "Accept", "Authorization", "Cookie"},
	}))

	app.Get("/ping", routes.Ping)

	app.Get("/logout", func(c fiber.Ctx) error {
		c.ClearCookie("AuthToken")
		return c.Redirect().Status(fiber.StatusMovedPermanently).To("/")
	})

	app.Post("/auth/login", func(c fiber.Ctx) error {
		return routes.Login(c, db)
	})
	app.Post("/auth/createAccount", func(c fiber.Ctx) error {
		return routes.CreateNewUser(c, db)
	})

	// shared static files from the sveltekit application

	app.Get("/auth/api/whoami", func(c fiber.Ctx) error {
		return routes.Whoami(c, db)
	})
	app.Get("/auth/api/getroles", func(c fiber.Ctx) error {
		return routes.GetRoles(c, db)
	})
	app.Get("/protected-api/ping", func(c fiber.Ctx) error {
		return routes.ProtectedPing(c, db)
	})

	// embedding all the static files into the binary
	subFS, err := fs.Sub(frontendFiles, "frontend_build")
	if err != nil {
		log.Panicln("Something very bad happened when packaging the Go Binary")
	}
	// static files
	app.Get("/*", static.New("", static.Config{
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
				f.Close()
				if content, readErr := fs.ReadFile(subFS, html); readErr == nil {
					c.Type("html")
					return c.Send(content)
				}
			}
			if content, readErr := fs.ReadFile(subFS, "404.html"); readErr == nil {
				c.Type("html")
				return c.Status(404).Send(content)
			}
			return routes.ErrorErrorError(c)
		},
	},
	))

	log.Fatal(app.Listen(":8000"))
}

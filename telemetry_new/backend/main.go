package main

import (
	"log"

	"hyped/backend_telemetry/internal/database"
	"hyped/backend_telemetry/internal/routes"
	"hyped/backend_telemetry/internal/testmode"

	"github.com/gofiber/fiber/v3"
	"github.com/gofiber/fiber/v3/middleware/cors"
	_ "github.com/lib/pq"
)

func main() {
	if _, err := database.Connect(database.PsqlConnectionString()); err != nil {
		log.Panicln("Cannot connect to the database!")
	}

	if err := routes.InitStatic(); err != nil {
		log.Panicln("Cannot initialize the static files")
	}

	testmode.TestCode()

	app := fiber.New()
	app.Use(cors.New())

	app.Use(cors.New(cors.Config{
		AllowOrigins: []string{"https://localhost:8080"},
		AllowHeaders: []string{"Origin", "Content-Type", "Accept", "Authorization", "Cookie"},
	}))

	app.Get("/ping", routes.Ping)

	app.Get("/auth/logout", routes.Logout)
	app.Post("/auth/login", routes.Login)
	app.Post("/auth/createAccount", routes.CreateNewUser)

	app.Get("/auth/api/whoami", routes.Whoami)
	app.Get("/auth/api/getroles", routes.GetRoles)
	app.Get("/protected-api/ping", routes.ProtectedPing)

	app.Get("/*", routes.StaticFiles)

	log.Fatal(app.Listen(":8000"))
}

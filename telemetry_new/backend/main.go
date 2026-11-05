package main

import (
    "log"
    "github.com/gofiber/fiber/v3"
    "github.com/gofiber/fiber/v3/middleware/cors"
	_ "github.com/lib/pq"
	// "hyped/backend_telemetry/internal/database"
)

func main() {
	// database.Connect(database.PsqlConnectionString())
    app := fiber.New()
	app.Use(cors.New())

	app.Use(cors.New(cors.Config{
		AllowOrigins: []string{"https://localhost:8080"},
		AllowHeaders: []string{"Origin", "Content-Type", "Accept"},
	}))
    app.Get("/", func(c fiber.Ctx) error {
        return c.SendString("This is the front page of the program")
    })

    app.Get("/ping", func(c fiber.Ctx) error {
        return c.SendString("pong")
    })

    log.Fatal(app.Listen(":8000"))
}

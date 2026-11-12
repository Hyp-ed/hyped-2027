package main

import (
	"log"
	"hyped/backend_telemetry/internal/database"
	"hyped/backend_telemetry/internal/routes"
	"github.com/gofiber/fiber/v3"
	"github.com/gofiber/fiber/v3/middleware/cors"
	_ "github.com/lib/pq"
)

func main() {
	db, err := database.Connect(database.PsqlConnectionString())
	if err != nil {
		log.Println("Cannot connect to the database!")
	}
	app := fiber.New()
	app.Use(cors.New())

	app.Use(cors.New(cors.Config{
		AllowOrigins: []string{"https://localhost:8080"},
		AllowHeaders: []string{"Origin", "Content-Type", "Accept"},
	}))
	app.Get("/", routes.Frontpage)

	app.Get("/ping", routes.Ping)

	app.Post("/auth/login", func (c fiber.Ctx) error {
		return routes.Login(c, db)
	})
	app.Post("/auth/createAccount", func (c fiber.Ctx) error {
		return routes.CreateNewUser(c, db)
	})
	
	log.Fatal(app.Listen(":8000"))
}

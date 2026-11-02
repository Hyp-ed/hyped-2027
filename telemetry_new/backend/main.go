package main

import (
	"os"
    "log"
    "github.com/gofiber/fiber/v3"
    "github.com/gofiber/fiber/v3/middleware/cors"
	"fmt"
	"database/sql"
	_ "github.com/lib/pq"
	"github.com/joho/godotenv"
)

func psqlConnectionString() string {
    if err := godotenv.Load(); err != nil {
		log.Println("The .env file are not loaded")
    }
	host := os.Getenv("POSTGRES_HOST")
	port := os.Getenv("POSTGRES_PORT")
	user := os.Getenv("POSTGRES_USER")
	password := os.Getenv("POSTGRES_PASSWORD")
	dbname := os.Getenv("POSTGRES_DB")
	return fmt.Sprintf("host=%s port=%d user=%s "+
    "password=%s dbname=%s sslmode=disable",
    host, port, user, password, dbname)
}

func main() {
	connectionString := psqlConnectionString()
	db, err := sql.Open("postgres", connectionString)
	if err != nil{
		log.Fatal("The database cannot be connected")
		log.Fatal(err)
	}
	rows, err := db.Query("SELECT 'Hello world';")
	if err != nil {
		log.Fatal("The database cannot be connected")
		log.Fatal(err)
	}
	println(rows)

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

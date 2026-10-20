package main

import (
    "log"
    "github.com/gofiber/fiber/v3"
)

func main() {
    app := fiber.New()

    app.Get("/", func(c fiber.Ctx) error {
        return c.SendString("This is the front page of the program")
    })

    app.Get("/ping", func(c fiber.Ctx) error {
        return c.SendString("pong")
    })

    log.Fatal(app.Listen(":8000"))
}

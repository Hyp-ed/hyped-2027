package routes

import "github.com/gofiber/fiber/v3"

func Frontpage(c fiber.Ctx) error {
	return c.SendString("This is the front page of the program")
}

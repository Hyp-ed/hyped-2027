package routes

import (
	"github.com/gofiber/fiber/v3"
)

func ErrorErrorError(c fiber.Ctx) error {
	return c.Status(404).SendString("Error 404")
}

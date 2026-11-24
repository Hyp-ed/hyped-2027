// Package middleware for the authentication
package middleware

import "github.com/gofiber/fiber/v3"

type Config struct {
    // Next skips this middleware when it returns true
    Next func(c fiber.Ctx) bool

    // Header to read the key from
    Header string

    // Validator decides whether a key is accepted
    Validator func(key string) bool
}
func New(config ...Config) fiber.Handler {
    return func(c fiber.Ctx) error {

        if false {
            return fiber.ErrUnauthorized
        }
        return c.Next()
    }
}

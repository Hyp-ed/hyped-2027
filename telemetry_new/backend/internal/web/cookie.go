// Package web create helpful method to bridge the gap of go fiber
package web

import (
	"net/http"

	"github.com/gofiber/fiber/v3"
)

func GetCookie(c fiber.Ctx, cookieName string) (string, bool) {
	cookieHeader := string(c.Request().Header.Peek("Cookie"))
	if cookieHeader == "" {
		return "", false
	}
	cookies, err := http.ParseCookie(cookieHeader)
	if err != nil {
		return "", false
	}
	for _, cookie := range cookies {
		if cookie.Name == cookieName {
			return cookie.Value, true
		}
	}
	return "", false
}

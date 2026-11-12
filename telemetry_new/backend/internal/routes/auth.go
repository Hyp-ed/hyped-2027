package routes

import (
	"database/sql"
	"log"
	"net/mail"
	"time"

	"hyped/backend_telemetry/internal/auth"
	"hyped/backend_telemetry/internal/database"

	"github.com/gofiber/fiber/v3"
)

type UserRequest struct {
	Email    string `json:"email"`
	Password string `json:"password"`
}

func Login(c fiber.Ctx, db *sql.DB) error {
	userRequest := new(UserRequest)
	if err := c.Bind().Body(userRequest); err != nil {
		log.Println(err.Error())
		return c.JSON(fiber.Map{"success": false})
	}
	if err := database.VerifyUser(db, userRequest.Email, userRequest.Password); err != nil {
		log.Println(err.Error())
		return c.JSON(fiber.Map{"success": false})
	}
	token, err := auth.CreateToken(userRequest.Email)
	if err != nil {
		log.Println(err.Error())
		return c.JSON(fiber.Map{"success": false})
	}
	expireTime := time.Now().Add(time.Hour * 120)
	c.Cookie(&fiber.Cookie{Expires: expireTime, Name: "AuthToken", Value: token, Secure: true, SameSite: "Lax"})
	return c.JSON(fiber.Map{"success": true})
}

type NewUserRequest struct {
	Email    string `json:"email"`
	Name     string `json:"name"`
	Password string `json:"password"`
}

func CreateNewUser(c fiber.Ctx, db *sql.DB) error {
	userRequest := new(NewUserRequest)
	if err := c.Bind().Body(userRequest); err != nil {
		log.Println(err.Error())
		return c.JSON(fiber.Map{"message": "Parse JSON failed", "success": false})
	}
	log.Println(userRequest.Email)
	_, err := mail.ParseAddress(userRequest.Email)
	if err != nil {
		log.Println(err.Error())
		return c.JSON(fiber.Map{"message": "Mail was not parsed", "success": false})
	}
	user, err := database.CreateUser(db, userRequest.Email, userRequest.Name, userRequest.Password)
	if err != nil {
		log.Println(err.Error())
		return c.JSON(fiber.Map{"message": "Database Error or duplicated email", "success": false})
	}
	return c.JSON(fiber.Map{"message": "New user created", "success": true, "userID": user.ID})
}

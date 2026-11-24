package routes

import (
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

func Login(c fiber.Ctx) error {
	userRequest := new(UserRequest)
	if err := c.Bind().Body(userRequest); err != nil {
		return c.JSON(fiber.Map{"success": false})
	}
	if err := database.VerifyUser(userRequest.Email, userRequest.Password); err != nil {
		return c.JSON(fiber.Map{"success": false})
	}
	token, err := auth.CreateToken(userRequest.Email)
	if err != nil {
		return c.JSON(fiber.Map{"success": false})
	}
	expireTime := time.Now().Add(time.Hour * 120)
	c.Cookie(&fiber.Cookie{Expires: expireTime, Name: "AuthToken", Value: token, Secure: false, SameSite: "Lax", Path: "/"})
	// PLEASE SET Secure to TRUE when deployed
	return c.JSON(fiber.Map{"success": true})
}

type NewUserRequest struct {
	Email    string `json:"email"`
	Name     string `json:"name"`
	Password string `json:"password"`
}

func CreateNewUser(c fiber.Ctx) error {
	userRequest := new(NewUserRequest)
	if err := c.Bind().Body(userRequest); err != nil {
		return c.JSON(fiber.Map{"message": "Parse JSON failed", "success": false})
	}
	_, err := mail.ParseAddress(userRequest.Email)
	if err != nil {
		return c.JSON(fiber.Map{"message": "Mail was not parsed", "success": false})
	}
	user, err := database.CreateUser(userRequest.Email, userRequest.Name, userRequest.Password)
	if err != nil {
		return c.JSON(fiber.Map{"message": "Database Error or duplicated email", "success": false})
	}
	return c.JSON(fiber.Map{"message": "New user created", "success": true, "userID": user.ID})
}

func Whoami(c fiber.Ctx) error {
	authHeader := c.Get("Authorization")
	email, err := auth.VerifyBearerToken(authHeader)
	if err != nil {
		return c.Status(400).JSON(fiber.Map{"user_exists": false, "success": true})
	}
	user,err := database.GetUserByEmail(email)
	if err != nil {
		return c.Status(400).JSON(fiber.Map{"user_exists": false, "success": true})
	}
	return c.Status(200).JSON(fiber.Map{"user_exists": true, "success": true, "name": user.Name, "email": email, "created": user.CreatedAt})
}

func GetRoles(c fiber.Ctx) error {
	roles, err := database.ListOfRoles()
	if err != nil {
		return c.Status(400).JSON(fiber.Map{"success": false})
	}
	return c.Status(200).JSON(fiber.Map{"success": true, "roles": roles})
}

func ProtectedPing(c fiber.Ctx) error {
	authHeader := c.Get("Authorization")
	email, err := auth.VerifyBearerToken(authHeader)
	if err != nil {
		return c.Status(400).JSON(fiber.Map{"success": false})
	}
	if database.CheckUserExistsByEmail(email) != nil {
		return c.Status(400).JSON(fiber.Map{"success": false})
	}
	return c.Status(200).JSON(fiber.Map{"response": "ping", "success": true})
}

func Logout(c fiber.Ctx) error {
	c.ClearCookie("AuthToken")
	return c.Redirect().Status(fiber.StatusMovedPermanently).To("/")
}

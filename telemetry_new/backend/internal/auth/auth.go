// Package auth provides authentication helper method for other to use
package auth

import (
	"hyped/backend_telemetry/internal/errors"
	"log"
	"time"

	"github.com/golang-jwt/jwt/v5"
	"golang.org/x/crypto/bcrypt"
	// Debug the sqlString
)

var secretKey = []byte("SUPER_SECRET_KEY_PLEASE_REPLACE_AT_PROD")

// PLEASE REPLACE AT PRODUCTION!!!!!!

// HashPassword creates a bcrypt hash from a plain-text password
func HashPassword(password string) (string, error) {
	// The cost determines how computationally expensive the hash is
	// Higher is more secure but slower (default is 10)
	hashedBytes, err := bcrypt.GenerateFromPassword([]byte(password), bcrypt.DefaultCost)
	if err != nil {
		return "", &errors.AuthError{ReasonFrom: "Cannot Hash Password"}
	}
	return string(hashedBytes), nil
}

// VerifyPassword checks if the provided password matches the stored hash
func VerifyPassword(hashedPassword string, providedPassword string) error {
	return bcrypt.CompareHashAndPassword([]byte(hashedPassword), []byte(providedPassword))
}

func CreateToken(email string) (string, error) {
	// token are signed using HMAC
	token := jwt.NewWithClaims(jwt.SigningMethodHS256, jwt.MapClaims{
		"email": email,
		"exp":   time.Now().Add(time.Hour * 24).Unix(),
	})
	tokenString, err := token.SignedString(secretKey)
	if err != nil {
		log.Println(err)
		return "", &errors.AuthError{ReasonFrom: "Cannot Create JWT Token"}
	}
	return tokenString, nil
}

func VerifyToken(tokenString string) error {
	token, err := jwt.Parse(tokenString, func(token *jwt.Token) (interface{}, error) {
		return secretKey, nil
	})
	if err != nil {
		return &errors.AuthError{ReasonFrom: "JWT Token Bad"}
	}else{
		if token.Valid {
			return nil
		}else{
			return &errors.AuthError{ReasonFrom: "JWT Token Bad"}
		}
	}
}

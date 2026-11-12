// Package database contains methods to help with database
package database

import (
	"database/sql"
	"log"
	"time"

	"hyped/backend_telemetry/internal/auth"
	"hyped/backend_telemetry/internal/errors"

	"github.com/google/uuid"
)

type User struct {
	ID           uuid.UUID
	Email        string
	PasswordHash string
	Name         string
	CreatedAt    time.Time
}

func CreateUser(db *sql.DB, email string, name string, password string) (*User, error) {
	passwordhash, err := auth.HashPassword(password)
	if err != nil {
		return nil, err
	}
	rows := db.QueryRow("INSERT INTO users (email, password_hash, name) VALUES ($1, $2, $3) RETURNING id, created_at;", email, passwordhash, name)
	user := User{Email: email, Name: name, PasswordHash: passwordhash}
	if err := rows.Scan(&user.ID, &user.CreatedAt); err != nil {
		return nil, &errors.UserError{ReasonFrom: "Cannot create a user"}
	} else {
		return &user, nil
	}
}

func DeleteUser(id uuid.UUID) error {
	return &errors.UserError{ReasonFrom: "Cannot delete a user"}
}

func ResetUserPassword(id uuid.UUID) (string, error) {
	return "", &errors.UserError{ReasonFrom: "Cannot reset a user's password"}
}

func VerifyUser(db *sql.DB, email string, password string) error {
	var passwordHash string
	rows := db.QueryRow(`SELECT users.password_hash FROM users WHERE users.email = $1;`, email)
	if err := rows.Scan(&passwordHash); err != nil {
		log.Println(err)
		return &errors.UserError{ReasonFrom: "Either email or password were wrong"}
	}
	if err := auth.VerifyPassword(passwordHash, password); err != nil {
		log.Println(err)
		return &errors.UserError{ReasonFrom: "Either email or password were wrong"}
	}
	return nil
}

func GetUserByEmail(db *sql.DB, email string) (*User, error) {
	user := User{}
	rows := db.QueryRow(`SELECT users.id, users.password_hash, users.email, users.name, users.created_at FROM users WHERE users.email = $1;`, email)
	if err := rows.Scan(&user.ID, &user.PasswordHash, &user.Email, &user.Name, &user.CreatedAt); err != nil {
		return nil, &errors.UserError{ReasonFrom: "Fail to find User"}
	} else {
		return &user, nil
	}
}

func CheckPermission(db *sql.DB, user *User, permission string) error {
	rows := db.QueryRow(`SELECT EXISTS
		(SELECT 1 FROM users 
		INNER JOIN users_role ON users_role.user_id = users.id 
		INNER JOIN role_permissions ON users_role.role_id = role_permissions.role_id
		INNER JOIN permissions ON role_permissions.permission_id = permissions.id
		WHERE permissions.name = $1 AND users.id = $2);`, permission, user.ID)

	var output bool
	if err := rows.Scan(&output); err != nil {
		log.Println("Error while reading for user")
		return &errors.UserError{ReasonFrom: "User doesn't exists or user doesn't have permission"}
	} else {
		return nil
	}
}

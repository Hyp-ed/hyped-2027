package users

import (
	"errors"
	"time"

	"github.com/google/uuid"
)

type User struct {
	ID           uuid.UUID
	Email        string
	PasswordHash string
	Name         string
	CreatedAt    time.Time
}

func CreateUser(email string, name string, passwordhash string) (*User, error) {
	return nil, errors.New("cannot create a user")
}

func DeleteUser(id uuid.UUID) error {
	return errors.New("cannot delete a user")
}

func ResetUserPassword(id uuid.UUID) (string, error) {
	return "", errors.New("cannot reset a user's password")
}

func GetUserByEmail(email string, passwordhash string) (*User, error) {
	return nil, errors.New("cannot get user by email and password")
}


// Package errors for AuthError provides an interface for Authentication errors
package errors

import "fmt"

type AuthError struct{
	ReasonFrom string
}

func (e *AuthError) Error() string {
	return fmt.Sprintln("Auth Error: %s", e.ReasonFrom)
}

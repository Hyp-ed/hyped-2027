package errors

import "fmt"

type UserError struct{
	ReasonFrom string
}

func (e *UserError) Error() string {
	return fmt.Sprintln("User Error: %s", e.ReasonFrom)
}

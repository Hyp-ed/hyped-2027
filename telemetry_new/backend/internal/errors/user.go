package errors

import "fmt"

type UserError struct{
	ReasonFrom string
}

func (e *UserError) Error() string {
	return fmt.Sprintf("User Error: %s", e.ReasonFrom)
}

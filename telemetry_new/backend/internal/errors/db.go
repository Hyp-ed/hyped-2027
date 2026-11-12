package errors

import "fmt"

type DBError struct{
	ReasonFrom string
	DBErrorMessage string
}

func (e *DBError) Error() string {
	return fmt.Sprintln("Database Error: %s, Reason: %s", e.ReasonFrom, e.DBErrorMessage)
}

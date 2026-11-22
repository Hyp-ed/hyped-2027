// Package testmode Please remove before deploying
package testmode

import (
	"hyped/backend_telemetry/internal/auth"
	"hyped/backend_telemetry/internal/database"
	"log"
	"os"
)

func TestCode() {
	// TESTING ONLY
	teston:= os.Getenv("TEST")
	if teston != "yes" {
		return
	}
	log.Println("TEST MODE IS ON")
	user, err := database.CreateUser("example@example.com", "John Testing", "testing")
	if err != nil {
		log.Println("User might have existed already!")
		user, err := database.GetUserByEmail("example@example.com")
		if err == nil {
			err := database.AddUserToRoles(user, "admin")
			if err != nil {
				log.Println(err.Error())
			}
		}
	} else {
		if database.AddUserToRoles(user, "admin") != nil {
			log.Fatalln("Error while creating test user")
		}
	}
	log.Println("test token: ")
	log.Println(auth.CreateToken("example@example.com"))
	// TESTING ONLY
}

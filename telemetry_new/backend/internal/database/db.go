package database

import (
	"database/sql"
	"fmt"
	"hyped/backend_telemetry/internal/errors"
	"log"
	"os"

	_ "github.com/jackc/pgx/v5/stdlib"
	"github.com/joho/godotenv"
)

func PsqlConnectionString() string {
	if err := godotenv.Load(); err != nil {
		log.Println("The .env file are not loaded")
	}
	host := os.Getenv("POSTGRES_HOST")
	port := "5432"
	user := os.Getenv("POSTGRES_USER")
	password := os.Getenv("POSTGRES_PASSWORD")
	dbname := os.Getenv("POSTGRES_DB")
	sqlString := fmt.Sprintf("host=%s port=%s user=%s "+
		"password=%s dbname=%s sslmode=disable",
		host, port, user, password, dbname)
	return sqlString
}

func Connect(connectionString string) (*sql.DB, error) {
	db, err := sql.Open("pgx", connectionString)
	if err != nil {
		return nil, &errors.DBError{ReasonFrom: "Database Connection Error", DBErrorMessage: err.Error()}
	}
	// Test the connection
	if err := db.Ping(); err != nil {
		return nil, &errors.DBError{ReasonFrom: "Database Ping Failed", DBErrorMessage: err.Error()}
	}
	log.Println("Database connected successfully")
	return db, nil
}


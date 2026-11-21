// Package database contains methods to help with database
package database

import (
	"database/sql"
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
		return &errors.UserError{ReasonFrom: "Either email or password were wrong"}
	}
	if err := auth.VerifyPassword(passwordHash, password); err != nil {
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

func CheckUserExistsByEmail(db *sql.DB, email string) error {
	rows := db.QueryRow(`SELECT EXISTS
		(SELECT 1 FROM users 
		WHERE users.email = $1);`, email)

	var output bool
	if err := rows.Scan(&output); err != nil {
		return &errors.UserError{ReasonFrom: "User doesn't exists"}
	} else {
		return nil
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
		return &errors.UserError{ReasonFrom: "User doesn't exists or user doesn't have permission"}
	} else {
		return nil
	}
}

func AddUserToRoles(db *sql.DB, user *User, roles string) error {
	_, err := db.Query(`INSERT INTO users_role (role_id, user_id)
SELECT roles.id, users.id FROM roles, users
WHERE users.id = $1 AND roles.name = $2;`, user.ID, roles)
	if err != nil {
		return &errors.UserError{ReasonFrom: "User or Roles doesn't exists or relation has been created already."}
	}
	return nil
}

type Permission struct {
	Name string
	ID   string
}

type Role struct {
	Name        string
	ID          string
	Permissions []Permission
}

func ListOfRoles(db *sql.DB) ([]Role, error) {
	var roles []Role
	rolesQuery, err := db.Query(`SELECT roles.id, roles.name FROM roles;`)
	if err != nil {
		return nil, &errors.DBError{ReasonFrom: "error while fetching for roles.", DBErrorMessage: err.Error()}
	}
	for rolesQuery.Next() {
		if rolesQuery.Err() != nil {
			return nil, &errors.DBError{ReasonFrom: "error while fetching for roles.", DBErrorMessage: err.Error()}
		}
		var role Role
		err := rolesQuery.Scan(&role.ID, &role.Name)
		if err != nil {
			return nil, &errors.DBError{ReasonFrom: "error while fetching for roles.", DBErrorMessage: err.Error()}
		}
		permissions, err := GetPermissionsByRole(db, role.ID)
		if err != nil {
			return nil, err
		}
		role.Permissions = permissions
		roles = append(roles, role)
	}
	err = rolesQuery.Close()
	if err != nil {
		return nil, &errors.DBError{ReasonFrom: "error while fetching for roles.", DBErrorMessage: err.Error()}
	}
	return roles, nil
}

func GetPermissionsByRole(db *sql.DB, roleID string) ([]Permission, error) {
	var permissions []Permission
	permissionsQuery, err := db.Query(`SELECT permissions.name, permissions.id 
			FROM role_permissions
			INNER JOIN permissions ON permissions.id = role_permissions.permission_id
			WHERE role_permissions.role_id = $1;`, roleID)
	if err != nil {
		return nil, &errors.DBError{ReasonFrom: "error while fetching for permissions.", DBErrorMessage: err.Error()}
	}
	for permissionsQuery.Next() {
		if permissionsQuery.Err() != nil {
			return nil, &errors.DBError{ReasonFrom: "error while fetching for permissions.", DBErrorMessage: err.Error()}
		}
		var permission Permission
		err := permissionsQuery.Scan(&permission.ID, &permission.Name)
		if err != nil {
			return nil, &errors.DBError{ReasonFrom: "error while fetching for permissions.", DBErrorMessage: err.Error()}
		}
		permissions = append(permissions, permission)
	}
	err = permissionsQuery.Close()
	if err != nil {
		return nil, &errors.DBError{ReasonFrom: "error while fetching for permissions.", DBErrorMessage: err.Error()}
	}
	return permissions, nil
}


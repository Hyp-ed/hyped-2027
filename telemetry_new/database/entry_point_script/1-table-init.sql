CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    password_hash VARCHAR(255),
    email VARCHAR(255) NOT NULL UNIQUE,
    name NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
)
CREATE TABLE roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(50) UNIQUE
);

CREATE TABLE permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) UNIQUE
);

CREATE TABLE role_permissions (
    role_id UUID PRIMARY KEY REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID PRIMARY KEY REFERENCES permissions(id)
)
CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token VARCHAR(255) UNIQUE NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    revoked BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE users_role (
    role_id UUID PRIMARY KEY REFERENCES roles(id) ON DELETE CASCADE,
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE
)
CREATE INDEX idx_refresh_tokens_token ON refresh_tokens(token);

INSERT INTO role_permissions (role_id, permission_id)
SELECT roles.id, permissions.id
FROM permissions, roles
WHERE permissions.name = 'admin' AND roles.name = 'admin';

INSERT INTO users_role (user_id, role_id)
SELECT users.id, roles.id
FROM users, roles
WHERE users.email = 'cephas8080@gmail.com' AND roles.name = 'admin';

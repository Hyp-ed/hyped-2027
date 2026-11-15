INSERT INTO permissions (name) VALUES 
    ('admin'),
    ('device'),
    ('user:edit_permissions'),
    ('user:reset_password'),
    ('user:create_user'),
    ('user:remove_user'),
    ('data:view_data');

INSERT INTO roles (name) VALUES
    ('admin'),
    ('device'),
    ('basic user');

INSERT INTO role_permissions (role_id, permission_id)
SELECT roles.id, permissions.id FROM roles, permissions
WHERE roles.name = 'admin' AND permissions.name = 'admin';

INSERT INTO role_permissions (role_id, permission_id)
SELECT roles.id, permissions.id FROM roles, permissions
WHERE roles.name = 'device' AND permissions.name = 'device';

INSERT INTO role_permissions (role_id, permission_id)
SELECT roles.id, permissions.id FROM roles, permissions
WHERE roles.name = 'basic user' AND permissions.name = 'data:view_data';

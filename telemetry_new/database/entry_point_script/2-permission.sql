INSERT INTO permission (name) VALUES 
    ('admin'),
    ('device'),
    ('user:reset_password'),
    ('user:create_user'),
    ('user:remove_user'),
    ('data:view_data');

INSERT INTO roles (name) VALUES
    ('admin'),
    ('device'),
    ('basic user');

INSERT INTO roles_permission (role_id)
    SELECT id FROM roles
    WHERE name=''

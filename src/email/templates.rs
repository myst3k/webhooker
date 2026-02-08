pub fn render_welcome(name: &str, base_url: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h2>Welcome to Webhooker</h2>
    <p>Hi {name},</p>
    <p>Your account has been created. You can log in at:</p>
    <p><a href="{base_url}" style="display: inline-block; padding: 10px 20px; background: #0070f3; color: white; text-decoration: none; border-radius: 4px;">Log In</a></p>
    <p style="color: #666; font-size: 14px;">If you didn't expect this email, you can ignore it.</p>
</body>
</html>"#
    )
}

pub fn render_password_reset(reset_url: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h2>Password Reset</h2>
    <p>A password reset was requested for your Webhooker account.</p>
    <p><a href="{reset_url}" style="display: inline-block; padding: 10px 20px; background: #0070f3; color: white; text-decoration: none; border-radius: 4px;">Reset Password</a></p>
    <p style="color: #666; font-size: 14px;">This link expires in 1 hour. If you didn't request this, you can ignore it.</p>
</body>
</html>"#
    )
}

pub fn render_member_added(name: &str, tenant_name: &str, base_url: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head><meta charset="utf-8"></head>
<body style="font-family: sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h2>You've been added to {tenant_name}</h2>
    <p>Hi {name},</p>
    <p>You've been added as a member of <strong>{tenant_name}</strong> on Webhooker.</p>
    <p><a href="{base_url}" style="display: inline-block; padding: 10px 20px; background: #0070f3; color: white; text-decoration: none; border-radius: 4px;">Log In</a></p>
</body>
</html>"#
    )
}

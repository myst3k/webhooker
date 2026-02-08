CREATE TABLE tenant_smtp_configs (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE UNIQUE,
    host VARCHAR(255) NOT NULL,
    port INTEGER NOT NULL,
    username_enc BYTEA NOT NULL,
    password_enc BYTEA NOT NULL,
    from_address VARCHAR(255) NOT NULL,
    from_name VARCHAR(255),
    tls_mode VARCHAR(10) NOT NULL DEFAULT 'starttls',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

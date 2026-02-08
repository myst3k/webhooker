use crate::models::{Endpoint, Project, Submission, Tenant};

pub struct ActionContext {
    pub submission: Submission,
    pub endpoint: Endpoint,
    pub project: Project,
    pub tenant: Tenant,
}

mod tools;

use picore_core::config::Paths;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{ServiceExt, tool, tool_router, transport::stdio};
use schemars::JsonSchema;
use serde::Deserialize;
use tools::Server;

#[derive(Deserialize, JsonSchema)]
struct ProjectArg {
    /// Absolute path of an indexed Project
    project: String,
}

#[derive(Deserialize, JsonSchema)]
struct SectionArg {
    /// Absolute path of an indexed Project
    project: String,
    /// README.md, CLAUDE.md or AGENTS.md
    file: String,
    heading: String,
}

#[derive(Deserialize, JsonSchema)]
struct HealthArg {
    /// Absolute path of an indexed Project, omit for machine checks only
    project: Option<String>,
}

struct Handler(Server);

fn text<T: serde::Serialize>(r: picore_core::error::Result<T>) -> Result<String, String> {
    r.map_err(|e| e.to_string())
        .and_then(|v| serde_json::to_string(&v).map_err(|e| e.to_string()))
}

#[tool_router(server_handler)]
impl Handler {
    #[tool(description = "List indexed Projects with branch, techs and Worktree count")]
    fn list_projects(&self) -> Result<String, String> {
        text(self.0.list_projects())
    }

    #[tool(description = "Outline of a Project's README.md, CLAUDE.md and AGENTS.md")]
    fn get_project_context(&self, Parameters(a): Parameters<ProjectArg>) -> Result<String, String> {
        text(self.0.get_project_context(&a.project))
    }

    #[tool(description = "Text of one heading section of a whitelisted Project file")]
    fn get_project_section(&self, Parameters(a): Parameters<SectionArg>) -> Result<String, String> {
        text(self.0.get_project_section(&a.project, &a.file, &a.heading))
    }

    #[tool(description = "Worktrees of a Project with prune verdict and staleness")]
    fn list_worktrees(&self, Parameters(a): Parameters<ProjectArg>) -> Result<String, String> {
        text(self.0.list_worktrees(&a.project))
    }

    #[tool(description = "Machine Health Checks, plus Project health when a project is given")]
    fn get_health(&self, Parameters(a): Parameters<HealthArg>) -> Result<String, String> {
        text(self.0.get_health(a.project.as_deref()))
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = Handler(Server::open(&Paths::from_env()?)?);
    server.serve(stdio()).await?.waiting().await?;
    Ok(())
}

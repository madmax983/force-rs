import re

with open("crates/force/src/api/bulk/ingest.rs", "r") as f:
    content = f.read()

# Replace first occurrence
old1 = """        let token = self.inner.token_manager.get_token_arc().await?;
        let mut url = format!(
            "{}/services/data/{}/jobs/ingest/{}",
            token.instance_url(),
            self.inner.config.api_version,
            self.job_id
        );"""

new1 = """        let mut url = self.inner.resolve_url(&format!("/jobs/ingest/{}", self.job_id)).await?;"""

if old1 in content:
    content = content.replace(old1, new1)
    print("Replaced first occurrence")
else:
    print("Could not find first occurrence")


# Replace second occurrence
old2 = """        let token = inner.token_manager.get_token_arc().await?;
        let url = format!(
            "{}/services/data/{}/jobs/ingest",
            token.instance_url(),
            inner.config.api_version
        );"""

new2 = """        let url = inner.resolve_url("/jobs/ingest").await?;"""

if old2 in content:
    content = content.replace(old2, new2)
    print("Replaced second occurrence")
else:
    print("Could not find second occurrence")

with open("crates/force/src/api/bulk/ingest.rs", "w") as f:
    f.write(content)

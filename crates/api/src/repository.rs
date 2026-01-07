use async_trait::async_trait;
use std::path::{Path, PathBuf};
use utils::rules::Rule;

use crate::error::{ApiError, Result};

/// Repository trait for managing transaction rules
#[async_trait]
pub trait RuleRepository: Send + Sync {
    async fn get_all_rules(&self) -> Result<Vec<Rule>>;
    async fn get_rule(&self, rule_id: &str) -> Result<Option<Rule>>;
    async fn create_rule(&self, rule: Rule) -> Result<()>;
    async fn update_rule(&self, rule: Rule) -> Result<()>;
    async fn delete_rule(&self, rule_id: &str) -> Result<()>;
}

/// File-based implementation that reads/writes rules from database.json
pub struct FileRuleRepository {
    database_path: PathBuf,
}

impl FileRuleRepository {
    pub fn new<P: AsRef<Path>>(database_path: P) -> Self {
        Self {
            database_path: database_path.as_ref().to_path_buf(),
        }
    }

    async fn load_database(&self) -> Result<serde_json::Value> {
        let content = tokio::fs::read_to_string(&self.database_path).await?;
        let database: serde_json::Value = serde_json::from_str(&content)?;
        Ok(database)
    }

    async fn save_database(&self, database: serde_json::Value) -> Result<()> {
        let content = serde_json::to_string_pretty(&database)?;
        tokio::fs::write(&self.database_path, content).await?;
        Ok(())
    }
}

#[async_trait]
impl RuleRepository for FileRuleRepository {
    async fn get_all_rules(&self) -> Result<Vec<Rule>> {
        let database = self.load_database().await?;
        let rules = database
            .get("rules")
            .and_then(|r| r.as_array())
            .ok_or_else(|| ApiError::InternalError("rules array not found".to_string()))?;

        let rules: Vec<Rule> = rules
            .iter()
            .filter_map(|r| serde_json::from_value(r.clone()).ok())
            .collect();

        Ok(rules)
    }

    async fn get_rule(&self, rule_id: &str) -> Result<Option<Rule>> {
        let rules = self.get_all_rules().await?;
        Ok(rules.into_iter().find(|r| r.rule_id == rule_id))
    }

    async fn create_rule(&self, rule: Rule) -> Result<()> {
        let mut database = self.load_database().await?;

        let rules = database
            .get_mut("rules")
            .and_then(|r| r.as_array_mut())
            .ok_or_else(|| ApiError::InternalError("rules array not found".to_string()))?;

        // Check if rule with same ID already exists
        if rules.iter().any(|r| {
            r.get("rule_id")
                .and_then(|id| id.as_str())
                .map(|id| id == rule.rule_id)
                .unwrap_or(false)
        }) {
            return Err(ApiError::BadRequest(format!(
                "Rule with ID {} already exists",
                rule.rule_id
            )));
        }

        let rule_value = serde_json::to_value(&rule)?;
        rules.push(rule_value);

        self.save_database(database).await
    }

    async fn update_rule(&self, rule: Rule) -> Result<()> {
        let mut database = self.load_database().await?;

        let rules = database
            .get_mut("rules")
            .and_then(|r| r.as_array_mut())
            .ok_or_else(|| ApiError::InternalError("rules array not found".to_string()))?;

        let rule_index = rules
            .iter()
            .position(|r| {
                r.get("rule_id")
                    .and_then(|id| id.as_str())
                    .map(|id| id == rule.rule_id)
                    .unwrap_or(false)
            })
            .ok_or_else(|| ApiError::NotFound(format!("Rule {} not found", rule.rule_id)))?;

        let rule_value = serde_json::to_value(&rule)?;
        rules[rule_index] = rule_value;

        self.save_database(database).await
    }

    async fn delete_rule(&self, rule_id: &str) -> Result<()> {
        let mut database = self.load_database().await?;

        let rules = database
            .get_mut("rules")
            .and_then(|r| r.as_array_mut())
            .ok_or_else(|| ApiError::InternalError("rules array not found".to_string()))?;

        let rule_index = rules
            .iter()
            .position(|r| {
                r.get("rule_id")
                    .and_then(|id| id.as_str())
                    .map(|id| id == rule_id)
                    .unwrap_or(false)
            })
            .ok_or_else(|| ApiError::NotFound(format!("Rule {} not found", rule_id)))?;

        rules.remove(rule_index);

        self.save_database(database).await
    }
}

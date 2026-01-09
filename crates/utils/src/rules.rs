use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Rule engine for processing transactions
/// Supports flexible conditions and actions

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub rule_id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub priority: i32, // Lower numbers = higher priority
    pub conditions: Vec<Condition>,
    pub condition_operator: ConditionOperator, // AND or OR
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    /// Description contains text (case-insensitive)
    DescriptionContains { value: String },
    
    /// Description matches regex
    DescriptionMatches { pattern: String },
    
    /// Description equals exactly
    DescriptionEquals { value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ConditionOperator {
    And,
    Or,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Set the category of the transaction
    SetCategory { category: String },
    
    /// Set the from account (for auto-routing)
    SetFromAccount { account_id: String },
    
    /// Set the to account (for auto-routing)
    SetToAccount { account_id: String },
    
    /// Set transaction type
    SetType { transaction_type: String },
}

impl Rule {
    /// Evaluates if the rule matches a transaction
    pub fn matches(&self, transaction: &Value) -> Result<bool> {
        if !self.enabled {
            return Ok(false);
        }

        if self.conditions.is_empty() {
            return Ok(false); // No conditions means no match
        }

        match self.condition_operator {
            ConditionOperator::And => {
                // All conditions must match
                for condition in &self.conditions {
                    if !condition.evaluate(transaction)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            ConditionOperator::Or => {
                // At least one condition must match
                for condition in &self.conditions {
                    if condition.evaluate(transaction)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    /// Applies the rule's actions to a transaction
    pub fn apply(&self, transaction: &mut Value) -> Result<Vec<String>> {
        let mut applied_actions = Vec::new();

        for action in &self.actions {
            let description = action.apply(transaction)?;
            applied_actions.push(description);
        }

        Ok(applied_actions)
    }
}

impl Condition {
    /// Evaluates a single condition against a transaction
    pub fn evaluate(&self, transaction: &Value) -> Result<bool> {
        match self {
            Condition::DescriptionContains { value } => {
                let desc = get_string_field(transaction, "description")?;
                Ok(desc.to_lowercase().contains(&value.to_lowercase()))
            }

            Condition::DescriptionMatches { pattern } => {
                let desc = get_string_field(transaction, "description")?;
                let regex = regex::Regex::new(pattern)
                    .map_err(|e| anyhow!("Invalid regex pattern: {}", e))?;
                Ok(regex.is_match(&desc))
            }

            Condition::DescriptionEquals { value } => {
                let desc = get_string_field(transaction, "description")?;
                Ok(desc.eq_ignore_ascii_case(value))
            }

        }
    }
}

impl Action {
    /// Applies an action to a transaction, returns description of what was done
    pub fn apply(&self, transaction: &mut Value) -> Result<String> {
        match self {
            Action::SetCategory { category } => {
                if let Some(obj) = transaction.as_object_mut() {
                    obj.insert("category".to_string(), Value::String(category.clone()));
                    Ok(format!("Set category to '{}'", category))
                } else {
                    Err(anyhow!("Transaction is not an object"))
                }
            }

            Action::SetFromAccount { account_id } => {
                if let Some(obj) = transaction.as_object_mut() {
                    obj.insert("from_account_id".to_string(), Value::String(account_id.clone()));
                    Ok(format!("Set from_account_id to '{}'", account_id))
                } else {
                    Err(anyhow!("Transaction is not an object"))
                }
            }

            Action::SetToAccount { account_id } => {
                if let Some(obj) = transaction.as_object_mut() {
                    obj.insert("to_account_id".to_string(), Value::String(account_id.clone()));
                    Ok(format!("Set to_account_id to '{}'", account_id))
                } else {
                    Err(anyhow!("Transaction is not an object"))
                }
            }

            Action::SetType { transaction_type } => {
                if let Some(obj) = transaction.as_object_mut() {
                    obj.insert("type".to_string(), Value::String(transaction_type.clone()));
                    Ok(format!("Set type to '{}'", transaction_type))
                } else {
                    Err(anyhow!("Transaction is not an object"))
                }
            }
        }
    }
}

/// Rule engine that manages and applies rules to transactions
pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// Apply rules to a transaction, returns descriptions of applied actions
    pub fn apply_rules(&self, transaction: &mut Value) -> Result<Vec<String>> {
        let mut all_actions = Vec::new();

        // Sort rules by priority (lower number = higher priority)
        let mut sorted_rules = self.rules.clone();
        sorted_rules.sort_by_key(|r| r.priority);

        for rule in &sorted_rules {
            if rule.matches(transaction)? {
                let actions = rule.apply(transaction)?;
                all_actions.extend(actions);
            }
        }

        Ok(all_actions)
    }

    /// Apply rules to multiple transactions
    pub fn apply_rules_batch(&self, transactions: &mut Vec<Value>) -> Result<Vec<(usize, Vec<String>)>> {
        let mut results = Vec::new();

        for (index, transaction) in transactions.iter_mut().enumerate() {
            let actions = self.apply_rules(transaction)?;
            if !actions.is_empty() {
                results.push((index, actions));
            }
        }

        Ok(results)
    }
}

// Helper functions

fn get_string_field(transaction: &Value, field: &str) -> Result<String> {
    transaction
        .get(field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Field '{}' not found or not a string", field))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_description_contains_rule() {
        let rule = Rule {
            rule_id: "1".to_string(),
            name: "RENT categorization".to_string(),
            description: Some("Categorize rent payments".to_string()),
            enabled: true,
            priority: 1,
            conditions: vec![Condition::DescriptionContains {
                value: "RENT".to_string(),
            }],
            condition_operator: ConditionOperator::And,
            actions: vec![Action::SetCategory {
                category: "housing".to_string(),
            }],
        };

        let mut transaction = json!({
            "description": "RENT payment",
            "category": "uncategorized",
            "amount": 8500.0,
        });

        assert!(rule.matches(&transaction).unwrap());
        rule.apply(&mut transaction).unwrap();
        assert_eq!(transaction["category"], "housing");
    }

    #[test]
    fn test_regex_pattern_rule() {
        let rule = Rule {
            rule_id: "2".to_string(),
            name: "Subscription categorization".to_string(),
            description: None,
            enabled: true,
            priority: 1,
            conditions: vec![
                Condition::DescriptionMatches {
                    pattern: "(NETFLIX|SPOTIFY|APPLE)".to_string(),
                },
            ],
            condition_operator: ConditionOperator::And,
            actions: vec![Action::SetCategory {
                category: "subscriptions".to_string(),
            }],
        };

        let mut transaction = json!({
            "description": "NETFLIX MONTHLY",
            "category": "uncategorized",
        });

        assert!(rule.matches(&transaction).unwrap());
        rule.apply(&mut transaction).unwrap();
        assert_eq!(transaction["category"], "subscriptions");
    }

    #[test]
    fn test_or_conditions() {
        let rule = Rule {
            rule_id: "3".to_string(),
            name: "Food categorization".to_string(),
            description: None,
            enabled: true,
            priority: 1,
            conditions: vec![
                Condition::DescriptionContains {
                    value: "FOODMARKET".to_string(),
                },
                Condition::DescriptionContains {
                    value: "HEMKÖPS".to_string(),
                },
            ],
            condition_operator: ConditionOperator::Or,
            actions: vec![Action::SetCategory {
                category: "food".to_string(),
            }],
        };

        let mut transaction1 = json!({
            "description": "FOODMARKET I",
            "category": "uncategorized",
        });

        let mut transaction2 = json!({
            "description": "HEMKÖPSKEDJA",
            "category": "uncategorized",
        });

        assert!(rule.matches(&transaction1).unwrap());
        assert!(rule.matches(&transaction2).unwrap());
        
        rule.apply(&mut transaction1).unwrap();
        rule.apply(&mut transaction2).unwrap();
        
        assert_eq!(transaction1["category"], "food");
        assert_eq!(transaction2["category"], "food");
    }
}

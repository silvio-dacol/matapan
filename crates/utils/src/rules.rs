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
    
    /// Amount comparison
    AmountGreaterThan { value: f64 },
    AmountLessThan { value: f64 },
    AmountEquals { value: f64 },
    AmountBetween { min: f64, max: f64 },
    
    /// Category conditions
    CategoryEquals { value: String },
    CategoryIn { values: Vec<String> },
    
    /// Type conditions
    TypeEquals { value: String }, // "income" | "expense" | "transfer"
    
    /// Account conditions
    FromAccountEquals { account_id: String },
    ToAccountEquals { account_id: String },
    
    /// Date conditions
    DateBefore { date: String },
    DateAfter { date: String },
    DateBetween { start: String, end: String },
    
    /// Currency condition
    CurrencyEquals { value: String },
    
    /// Custom field conditions (for future extensibility)
    CustomFieldContains { field: String, value: String },
    CustomFieldEquals { field: String, value: String },
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
    
    /// Add a tag (if we implement tags in the future)
    AddTag { tag: String },
    
    /// Set a custom field
    SetField { field: String, value: String },
    
    /// Set the from account (for auto-routing)
    SetFromAccount { account_id: String },
    
    /// Set the to account (for auto-routing)
    SetToAccount { account_id: String },
    
    /// Mark for review
    MarkForReview { reason: String },
    
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

            Condition::AmountGreaterThan { value } => {
                let amount = get_number_field(transaction, "amount")?;
                Ok(amount > *value)
            }

            Condition::AmountLessThan { value } => {
                let amount = get_number_field(transaction, "amount")?;
                Ok(amount < *value)
            }

            Condition::AmountEquals { value } => {
                let amount = get_number_field(transaction, "amount")?;
                Ok((amount - value).abs() < 0.01) // Floating point comparison
            }

            Condition::AmountBetween { min, max } => {
                let amount = get_number_field(transaction, "amount")?;
                Ok(amount >= *min && amount <= *max)
            }

            Condition::CategoryEquals { value } => {
                let category = get_string_field(transaction, "category")?;
                Ok(category == *value)
            }

            Condition::CategoryIn { values } => {
                let category = get_string_field(transaction, "category")?;
                Ok(values.contains(&category))
            }

            Condition::TypeEquals { value } => {
                let txn_type = get_string_field(transaction, "type")?;
                Ok(txn_type == *value)
            }

            Condition::FromAccountEquals { account_id } => {
                let from_account = get_string_field(transaction, "from_account_id")?;
                Ok(from_account == *account_id)
            }

            Condition::ToAccountEquals { account_id } => {
                let to_account = get_string_field(transaction, "to_account_id")?;
                Ok(to_account == *account_id)
            }

            Condition::DateBefore { date } => {
                let txn_date = get_string_field(transaction, "date")?;
                Ok(txn_date < *date)
            }

            Condition::DateAfter { date } => {
                let txn_date = get_string_field(transaction, "date")?;
                Ok(txn_date > *date)
            }

            Condition::DateBetween { start, end } => {
                let txn_date = get_string_field(transaction, "date")?;
                Ok(txn_date >= *start && txn_date <= *end)
            }

            Condition::CurrencyEquals { value } => {
                let currency = get_string_field(transaction, "currency")?;
                Ok(currency == *value)
            }

            Condition::CustomFieldContains { field, value } => {
                let field_value = get_string_field(transaction, field)?;
                Ok(field_value.to_lowercase().contains(&value.to_lowercase()))
            }

            Condition::CustomFieldEquals { field, value } => {
                let field_value = get_string_field(transaction, field)?;
                Ok(field_value == *value)
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

            Action::AddTag { tag } => {
                if let Some(obj) = transaction.as_object_mut() {
                    let tags = obj
                        .entry("tags")
                        .or_insert_with(|| Value::Array(vec![]));
                    
                    if let Some(tags_array) = tags.as_array_mut() {
                        if !tags_array.contains(&Value::String(tag.clone())) {
                            tags_array.push(Value::String(tag.clone()));
                        }
                    }
                    Ok(format!("Added tag '{}'", tag))
                } else {
                    Err(anyhow!("Transaction is not an object"))
                }
            }

            Action::SetField { field, value } => {
                if let Some(obj) = transaction.as_object_mut() {
                    obj.insert(field.clone(), Value::String(value.clone()));
                    Ok(format!("Set field '{}' to '{}'", field, value))
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

            Action::MarkForReview { reason } => {
                if let Some(obj) = transaction.as_object_mut() {
                    obj.insert("needs_review".to_string(), Value::Bool(true));
                    obj.insert("review_reason".to_string(), Value::String(reason.clone()));
                    Ok(format!("Marked for review: {}", reason))
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

fn get_number_field(transaction: &Value, field: &str) -> Result<f64> {
    transaction
        .get(field)
        .and_then(|v| v.as_f64())
        .ok_or_else(|| anyhow!("Field '{}' not found or not a number", field))
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
    fn test_amount_range_rule() {
        let rule = Rule {
            rule_id: "2".to_string(),
            name: "Large expense alert".to_string(),
            description: None,
            enabled: true,
            priority: 1,
            conditions: vec![
                Condition::AmountGreaterThan { value: 10000.0 },
                Condition::TypeEquals {
                    value: "expense".to_string(),
                },
            ],
            condition_operator: ConditionOperator::And,
            actions: vec![Action::MarkForReview {
                reason: "Large expense".to_string(),
            }],
        };

        let mut transaction = json!({
            "description": "Big purchase",
            "type": "expense",
            "amount": 15000.0,
        });

        assert!(rule.matches(&transaction).unwrap());
        rule.apply(&mut transaction).unwrap();
        assert_eq!(transaction["needs_review"], true);
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

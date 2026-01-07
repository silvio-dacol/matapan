use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use utils::rules::{Rule, RuleEngine};

use crate::{error::ApiError, repository::RuleRepository, Result};

pub type RuleRepositoryState = Arc<dyn RuleRepository>;

/// GET /api/rules
/// Returns all rules
pub async fn get_rules(State(repo): State<RuleRepositoryState>) -> Result<impl IntoResponse> {
    let rules = repo.get_all_rules().await?;
    Ok(Json(rules))
}

/// GET /api/rules/:rule_id
/// Returns a specific rule by ID
pub async fn get_rule(
    State(repo): State<RuleRepositoryState>,
    Path(rule_id): Path<String>,
) -> Result<impl IntoResponse> {
    let rule = repo
        .get_rule(&rule_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Rule {} not found", rule_id)))?;
    Ok(Json(rule))
}

/// POST /api/rules
/// Creates a new rule
pub async fn create_rule(
    State(repo): State<RuleRepositoryState>,
    Json(rule): Json<Rule>,
) -> Result<impl IntoResponse> {
    repo.create_rule(rule.clone()).await?;
    Ok((StatusCode::CREATED, Json(rule)))
}

/// PUT /api/rules/:rule_id
/// Updates an existing rule
pub async fn update_rule(
    State(repo): State<RuleRepositoryState>,
    Path(rule_id): Path<String>,
    Json(rule): Json<Rule>,
) -> Result<impl IntoResponse> {
    if rule.rule_id != rule_id {
        return Err(ApiError::BadRequest(
            "Rule ID in path does not match rule ID in body".to_string(),
        ));
    }

    repo.update_rule(rule.clone()).await?;
    Ok(Json(rule))
}

/// DELETE /api/rules/:rule_id
/// Deletes a rule
pub async fn delete_rule(
    State(repo): State<RuleRepositoryState>,
    Path(rule_id): Path<String>,
) -> Result<impl IntoResponse> {
    repo.delete_rule(&rule_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/rules/test
/// Test a rule against sample transaction without saving
#[derive(Debug, Deserialize)]
pub struct TestRuleRequest {
    rule: Rule,
    transaction: Value,
}

#[derive(Debug, Serialize)]
pub struct TestRuleResponse {
    matches: bool,
    modified_transaction: Option<Value>,
    applied_actions: Vec<String>,
}

pub async fn test_rule(Json(req): Json<TestRuleRequest>) -> Result<impl IntoResponse> {
    let matches = req.rule.matches(&req.transaction)?;

    let mut modified_transaction = req.transaction.clone();
    let applied_actions = if matches {
        req.rule.apply(&mut modified_transaction)?
    } else {
        vec![]
    };

    let response = TestRuleResponse {
        matches,
        modified_transaction: if matches {
            Some(modified_transaction)
        } else {
            None
        },
        applied_actions,
    };

    Ok(Json(response))
}

/// POST /api/rules/apply
/// Apply all rules to a set of transactions
#[derive(Debug, Deserialize)]
pub struct ApplyRulesRequest {
    transactions: Vec<Value>,
}

#[derive(Debug, Serialize)]
pub struct ApplyRulesResponse {
    modified_transactions: Vec<Value>,
    results: Vec<TransactionRuleResult>,
}

#[derive(Debug, Serialize)]
pub struct TransactionRuleResult {
    transaction_index: usize,
    txn_id: Option<String>,
    applied_actions: Vec<String>,
}

pub async fn apply_rules(
    State(repo): State<RuleRepositoryState>,
    Json(req): Json<ApplyRulesRequest>,
) -> Result<impl IntoResponse> {
    let rules = repo.get_all_rules().await?;
    let engine = RuleEngine::new(rules);

    let mut modified_transactions = req.transactions.clone();
    let batch_results = engine.apply_rules_batch(&mut modified_transactions)?;

    let results: Vec<TransactionRuleResult> = batch_results
        .into_iter()
        .map(|(index, actions)| {
            let txn_id = modified_transactions
                .get(index)
                .and_then(|t| t.get("txn_id"))
                .and_then(|id| id.as_str())
                .map(|s| s.to_string());

            TransactionRuleResult {
                transaction_index: index,
                txn_id,
                applied_actions: actions,
            }
        })
        .collect();

    let response = ApplyRulesResponse {
        modified_transactions,
        results,
    };

    Ok(Json(response))
}

/// POST /api/rules/reorder
/// Reorder rules by updating priorities
#[derive(Debug, Deserialize)]
pub struct ReorderRulesRequest {
    rule_ids: Vec<String>, // Ordered list of rule IDs
}

pub async fn reorder_rules(
    State(repo): State<RuleRepositoryState>,
    Json(req): Json<ReorderRulesRequest>,
) -> Result<impl IntoResponse> {
    // Update priorities based on order (index becomes priority)
    for (index, rule_id) in req.rule_ids.iter().enumerate() {
        if let Some(mut rule) = repo.get_rule(rule_id).await? {
            rule.priority = index as i32;
            repo.update_rule(rule).await?;
        }
    }

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Rules reordered successfully"
    })))
}

/// POST /api/rules/:rule_id/toggle
/// Toggle a rule's enabled status
pub async fn toggle_rule(
    State(repo): State<RuleRepositoryState>,
    Path(rule_id): Path<String>,
) -> Result<impl IntoResponse> {
    let mut rule = repo
        .get_rule(&rule_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Rule {} not found", rule_id)))?;

    rule.enabled = !rule.enabled;
    repo.update_rule(rule.clone()).await?;

    Ok(Json(rule))
}

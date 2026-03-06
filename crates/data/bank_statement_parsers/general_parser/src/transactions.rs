use ai_client::OllamaClient;
use anyhow::{anyhow, Context, Result};
use calamine::{open_workbook_auto, Reader};
use encoding_rs::GB18030;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use utils::{build_transaction, TransactionInput};

use crate::GeneralParser;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatIssue {
    pub level: IssueLevel,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnfoldedSheet {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnfoldedInput {
    pub file_name: String,
    pub file_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    pub sheets: Vec<UnfoldedSheet>,
    pub issues: Vec<FormatIssue>,
}

#[derive(Debug, Clone)]
pub struct ParseTransactionsOutput {
    pub transactions: Vec<Value>,
    pub used_account_ids: Vec<String>,
    pub issues: Vec<FormatIssue>,
}

#[derive(Debug, Clone, Deserialize)]
struct AiAccountDraft {
    account_id: String,
}

#[derive(Debug, Clone, Deserialize)]
struct AiTransactionDraft {
    date: String,
    amount: f64,
    currency: Option<String>,
    description: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    transaction_type: Option<String>,
    #[serde(default)]
    from_account_id: Option<String>,
    #[serde(default)]
    to_account_id: Option<String>,
    #[serde(default)]
    txn_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AiParsePayload {
    #[serde(default)]
    accounts: Vec<AiAccountDraft>,
    #[serde(default)]
    transactions: Vec<AiTransactionDraft>,
    #[serde(default)]
    issues: Vec<FormatIssue>,
}

pub fn parse_transactions(
    parser: &GeneralParser,
    ai: &OllamaClient,
    input_file_path: &str,
) -> Result<ParseTransactionsOutput> {
    let unfolded = unfold_input_file(input_file_path)?;
    let mut issues = unfolded.issues.clone();

    let ai_payload = extract_with_ai(
        ai,
        &unfolded,
        &parser.account_id,
        &parser.default_currency,
        &parser.institution,
    )?;
    issues.extend(ai_payload.issues.clone());

    let mut used_account_ids = HashSet::new();
    used_account_ids.insert(parser.account_id.clone());

    for account in &ai_payload.accounts {
        let account_id = normalize_account_id(&account.account_id);
        if !account_id.is_empty() {
            used_account_ids.insert(account_id);
        }
    }

    let transactions = ai_payload_to_transactions(
        ai_payload,
        &parser.account_id,
        &parser.default_currency,
        input_file_path,
        &mut issues,
        &mut used_account_ids,
    );

    Ok(ParseTransactionsOutput {
        transactions,
        used_account_ids: used_account_ids.into_iter().collect(),
        issues,
    })
}

pub fn unfold_input_file(path: &str) -> Result<UnfoldedInput> {
    let extension = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match extension.as_str() {
        "csv" => unfold_csv_file(path),
        "xls" | "xlsx" => unfold_excel_file(path),
        _ => Err(anyhow!(
            "Unsupported file format for generic parser: {}",
            path
        )),
    }
}

fn unfold_csv_file(path: &str) -> Result<UnfoldedInput> {
    let raw = fs::read(path).with_context(|| format!("Cannot read CSV file {}", path))?;

    let (decoded_text, encoding, mut issues) = decode_csv_text(&raw, path)?;
    let (rows, headers, delimiter, parse_issues) = parse_csv_rows(&decoded_text, path)?;
    issues.extend(parse_issues);

    if rows.is_empty() {
        issues.push(FormatIssue {
            level: IssueLevel::Warning,
            code: "empty_csv".to_string(),
            message: "CSV has no data rows".to_string(),
            file: Some(path.to_string()),
            sheet: Some("csv".to_string()),
            row: None,
        });
    }

    issues.push(FormatIssue {
        level: IssueLevel::Info,
        code: "csv_delimiter_detected".to_string(),
        message: format!("Detected delimiter '{}'", delimiter),
        file: Some(path.to_string()),
        sheet: Some("csv".to_string()),
        row: None,
    });

    Ok(UnfoldedInput {
        file_name: path.to_string(),
        file_format: "csv".to_string(),
        encoding: Some(encoding),
        sheets: vec![UnfoldedSheet {
            name: "csv".to_string(),
            headers,
            rows,
        }],
        issues,
    })
}

fn unfold_excel_file(path: &str) -> Result<UnfoldedInput> {
    let mut workbook = open_workbook_auto(path).with_context(|| {
        format!(
            "Cannot open Excel file {}. If it is password-protected/encrypted, export a plain CSV/XLSX first.",
            path
        )
    })?;

    let mut sheets = Vec::new();
    let mut issues = Vec::new();

    let sheet_names = workbook.sheet_names().to_vec();
    for sheet_name in sheet_names {
        let range = match workbook.worksheet_range(&sheet_name) {
            Ok(r) => r,
            Err(err) => {
                issues.push(FormatIssue {
                    level: IssueLevel::Warning,
                    code: "excel_sheet_unreadable".to_string(),
                    message: format!("Cannot read sheet '{}': {}", sheet_name, err),
                    file: Some(path.to_string()),
                    sheet: Some(sheet_name.clone()),
                    row: None,
                });
                continue;
            }
        };

        let mut rows_iter = range.rows();
        let Some(header_row) = rows_iter.next() else {
            continue;
        };

        let mut headers: Vec<String> = header_row
            .iter()
            .enumerate()
            .map(|(idx, cell)| {
                let value = normalize_text(&cell.to_string());
                if value.is_empty() {
                    format!("col_{}", idx + 1)
                } else {
                    value
                }
            })
            .collect();

        headers = uniquify_headers(headers);

        let mut rows = Vec::new();
        for (row_idx, row) in rows_iter.enumerate() {
            let mut row_obj = serde_json::Map::new();
            let mut non_empty = false;

            for (col_idx, value) in row.iter().enumerate() {
                let key = headers
                    .get(col_idx)
                    .cloned()
                    .unwrap_or_else(|| format!("col_{}", col_idx + 1));

                let cell_text = normalize_text(&value.to_string());
                if !cell_text.is_empty() {
                    non_empty = true;
                }

                row_obj.insert(key, Value::String(cell_text));
            }

            if non_empty {
                rows.push(Value::Object(row_obj));
            } else if row_idx < 3 {
                issues.push(FormatIssue {
                    level: IssueLevel::Info,
                    code: "excel_empty_row".to_string(),
                    message: "Skipped empty row near top of sheet".to_string(),
                    file: Some(path.to_string()),
                    sheet: Some(sheet_name.clone()),
                    row: Some(row_idx + 2),
                });
            }
        }

        sheets.push(UnfoldedSheet {
            name: sheet_name,
            headers,
            rows,
        });
    }

    if sheets.is_empty() {
        issues.push(FormatIssue {
            level: IssueLevel::Error,
            code: "excel_no_readable_sheets".to_string(),
            message: "No readable sheets found in workbook".to_string(),
            file: Some(path.to_string()),
            sheet: None,
            row: None,
        });
    }

    Ok(UnfoldedInput {
        file_name: path.to_string(),
        file_format: "excel".to_string(),
        encoding: None,
        sheets,
        issues,
    })
}

fn decode_csv_text(raw: &[u8], path: &str) -> Result<(String, String, Vec<FormatIssue>)> {
    if let Ok(text) = std::str::from_utf8(raw) {
        return Ok((
            text.to_string(),
            "utf-8".to_string(),
            vec![FormatIssue {
                level: IssueLevel::Info,
                code: "csv_encoding_detected".to_string(),
                message: "Decoded as UTF-8".to_string(),
                file: Some(path.to_string()),
                sheet: Some("csv".to_string()),
                row: None,
            }],
        ));
    }

    let (decoded_gb, _, gb_had_errors) = GB18030.decode(raw);
    if !gb_had_errors {
        return Ok((
            decoded_gb.into_owned(),
            "gb18030".to_string(),
            vec![FormatIssue {
                level: IssueLevel::Warning,
                code: "csv_non_utf8_decoded".to_string(),
                message: "CSV decoded as GB18030".to_string(),
                file: Some(path.to_string()),
                sheet: Some("csv".to_string()),
                row: None,
            }],
        ));
    }

    let lossy = String::from_utf8_lossy(raw).into_owned();
    Ok((
        lossy,
        "utf-8-lossy".to_string(),
        vec![FormatIssue {
            level: IssueLevel::Warning,
            code: "csv_lossy_decode".to_string(),
            message: "CSV required lossy UTF-8 decoding; some characters may be corrupted"
                .to_string(),
            file: Some(path.to_string()),
            sheet: Some("csv".to_string()),
            row: None,
        }],
    ))
}

fn parse_csv_rows(
    text: &str,
    path: &str,
) -> Result<(Vec<Value>, Vec<String>, char, Vec<FormatIssue>)> {
    let delimiters = [',', ';', '\t', '|'];
    let mut best_rows = Vec::new();
    let mut best_headers = Vec::new();
    let mut best_delimiter = ',';
    let mut best_score = 0usize;
    let mut issues = Vec::new();

    for delimiter in delimiters {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(delimiter as u8)
            .trim(csv::Trim::All)
            .flexible(true)
            .from_reader(text.as_bytes());

        let headers = match reader.headers() {
            Ok(h) => h.clone(),
            Err(_) => continue,
        };

        let mut parsed_rows = Vec::new();
        let mut row_parse_errors = 0usize;
        let mut field_populated = 0usize;

        let normalized_headers: Vec<String> = uniquify_headers(
            headers
                .iter()
                .enumerate()
                .map(|(idx, h)| {
                    let n = normalize_text(h);
                    if n.is_empty() {
                        format!("col_{}", idx + 1)
                    } else {
                        n
                    }
                })
                .collect(),
        );

        for (row_idx, record_result) in reader.records().enumerate() {
            match record_result {
                Ok(record) => {
                    let mut row_obj = serde_json::Map::new();
                    let mut non_empty = false;

                    for (idx, value) in record.iter().enumerate() {
                        let key = normalized_headers
                            .get(idx)
                            .cloned()
                            .unwrap_or_else(|| format!("col_{}", idx + 1));
                        let normalized_value = normalize_text(value);
                        if !normalized_value.is_empty() {
                            non_empty = true;
                            field_populated += 1;
                        }
                        row_obj.insert(key, Value::String(normalized_value));
                    }

                    if non_empty {
                        parsed_rows.push(Value::Object(row_obj));
                    } else if row_idx < 3 {
                        issues.push(FormatIssue {
                            level: IssueLevel::Info,
                            code: "csv_empty_row".to_string(),
                            message: "Skipped empty row near top of file".to_string(),
                            file: Some(path.to_string()),
                            sheet: Some("csv".to_string()),
                            row: Some(row_idx + 2),
                        });
                    }
                }
                Err(_) => row_parse_errors += 1,
            }
        }

        let score = normalized_headers.len() * 1000 + field_populated;
        if row_parse_errors == 0 && score > best_score {
            best_score = score;
            best_rows = parsed_rows;
            best_headers = normalized_headers;
            best_delimiter = delimiter;
        }
    }

    if best_headers.is_empty() {
        return Err(anyhow!(
            "Cannot parse CSV file {} with common delimiters",
            path
        ));
    }

    Ok((best_rows, best_headers, best_delimiter, issues))
}

fn extract_with_ai(
    ai: &OllamaClient,
    unfolded: &UnfoldedInput,
    parser_account_id: &str,
    default_currency: &str,
    default_institution: &str,
) -> Result<AiParsePayload> {
    let feedback = format_issues_feedback_for_ai(&unfolded.issues);
    let unfolded_json = serde_json::to_string(unfolded)?;

    let system_prompt = r#"You are a strict financial statement normalizer.
You must output ONLY valid JSON with this shape:
{
  "accounts": [{ "account_id": "STRING" }],
  "transactions": [
    {
      "date": "YYYY-MM-DD",
      "amount": 123.45,
      "currency": "ISO4217",
      "description": "STRING",
      "category": "STRING",
      "transaction_type": "expense|income|internal_transfer",
      "from_account_id": "STRING",
      "to_account_id": "STRING",
      "txn_id": "STRING"
    }
  ],
  "issues": [
    {
      "level": "info|warning|error",
      "code": "STRING",
      "message": "STRING",
      "file": "STRING or null",
      "sheet": "STRING or null",
      "row": 1
    }
  ]
}
Rules:
- Keep values deterministic and conservative.
- If uncertain, still provide a transaction with best effort and add an issue.
- Never return markdown or prose.
"#;

    let user_prompt = format!(
        "Normalize this unfolded statement into Matapan transaction elements.\n\
Default account_id: {parser_account_id}\n\
Default institution: {default_institution}\n\
Default currency: {default_currency}\n\
Input format feedback (important):\n{feedback}\n\
Unfolded input JSON:\n{unfolded_json}"
    );

    let raw = ai.chat(system_prompt, &user_prompt)?;
    let json_text = extract_first_json_object(&raw)
        .ok_or_else(|| anyhow!("AI response does not contain a valid JSON object"))?;

    serde_json::from_str::<AiParsePayload>(json_text)
        .context("Cannot parse AI normalization JSON payload")
}

fn extract_first_json_object(text: &str) -> Option<&str> {
    if let Some(start) = text.find('{') {
        let mut depth = 0i32;
        let mut in_string = false;
        let mut escaped = false;

        for (offset, ch) in text[start..].char_indices() {
            if escaped {
                escaped = false;
                continue;
            }

            match ch {
                '\\' if in_string => escaped = true,
                '"' => in_string = !in_string,
                '{' if !in_string => depth += 1,
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(&text[start..start + offset + 1]);
                    }
                }
                _ => {}
            }
        }
    }

    None
}

fn format_issues_feedback_for_ai(issues: &[FormatIssue]) -> String {
    if issues.is_empty() {
        return "No pre-detected format issues.".to_string();
    }

    let mut out = String::new();
    for issue in issues {
        let file = issue.file.clone().unwrap_or_else(|| "unknown".to_string());
        let sheet = issue.sheet.clone().unwrap_or_else(|| "unknown".to_string());
        let row = issue
            .row
            .map(|r| r.to_string())
            .unwrap_or_else(|| "n/a".to_string());

        out.push_str(&format!(
            "- level={:?}, code={}, file={}, sheet={}, row={}, message={}\n",
            issue.level, issue.code, file, sheet, row, issue.message
        ));
    }
    out
}

fn ai_payload_to_transactions(
    payload: AiParsePayload,
    parser_account_id: &str,
    default_currency: &str,
    file_path: &str,
    issues: &mut Vec<FormatIssue>,
    used_account_ids: &mut HashSet<String>,
) -> Vec<Value> {
    let mut out = Vec::new();

    for (idx, draft) in payload.transactions.into_iter().enumerate() {
        let date = draft.date.trim().to_string();
        if !looks_like_iso_date(&date) {
            issues.push(FormatIssue {
                level: IssueLevel::Warning,
                code: "invalid_date_normalized".to_string(),
                message: format!("AI produced non-ISO date '{}'", date),
                file: Some(file_path.to_string()),
                sheet: None,
                row: Some(idx + 1),
            });
        }

        let mut tx_type = draft
            .transaction_type
            .unwrap_or_else(|| infer_type_from_amount(draft.amount).to_string());
        if tx_type != "expense" && tx_type != "income" && tx_type != "internal_transfer" {
            issues.push(FormatIssue {
                level: IssueLevel::Warning,
                code: "invalid_transaction_type".to_string(),
                message: format!(
                    "Unsupported transaction type '{}', fallback to expense/income",
                    tx_type
                ),
                file: Some(file_path.to_string()),
                sheet: None,
                row: Some(idx + 1),
            });
            tx_type = infer_type_from_amount(draft.amount).to_string();
        }

        let from_account_id = draft
            .from_account_id
            .map(|s| normalize_account_id(&s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_from_account(parser_account_id, &tx_type));

        let to_account_id = draft
            .to_account_id
            .map(|s| normalize_account_id(&s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_to_account(parser_account_id, &tx_type));

        used_account_ids.insert(from_account_id.clone());
        used_account_ids.insert(to_account_id.clone());

        let amount_abs = draft.amount.abs();
        if amount_abs == 0.0 {
            issues.push(FormatIssue {
                level: IssueLevel::Warning,
                code: "zero_amount_transaction".to_string(),
                message: "Skipping zero amount transaction".to_string(),
                file: Some(file_path.to_string()),
                sheet: None,
                row: Some(idx + 1),
            });
            continue;
        }

        let description = normalize_text(&draft.description);
        let category = draft
            .category
            .map(|s| normalize_text(&s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "uncategorized".to_string());

        let txn_id = draft
            .txn_id
            .map(|s| normalize_text(&s))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                make_txn_id(
                    &date,
                    &from_account_id,
                    &to_account_id,
                    amount_abs,
                    draft.currency.as_deref().unwrap_or(default_currency),
                    &description,
                    idx,
                )
            });

        out.push(build_transaction(&TransactionInput {
            date,
            from_account_id,
            to_account_id,
            transaction_type: tx_type,
            category,
            amount: amount_abs,
            currency: draft
                .currency
                .map(|s| normalize_text(&s).to_uppercase())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| default_currency.to_uppercase()),
            description,
            description_en: None,
            txn_id,
        }));
    }

    if out.is_empty() {
        issues.push(FormatIssue {
            level: IssueLevel::Error,
            code: "no_transactions_produced".to_string(),
            message: "AI parsing did not produce any transactions".to_string(),
            file: Some(file_path.to_string()),
            sheet: None,
            row: None,
        });
    }

    out
}

fn normalize_account_id(raw: &str) -> String {
    normalize_text(raw)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn infer_type_from_amount(amount: f64) -> &'static str {
    if amount >= 0.0 {
        "expense"
    } else {
        "income"
    }
}

fn looks_like_iso_date(date: &str) -> bool {
    date.len() == 10
        && date.chars().enumerate().all(|(idx, ch)| match idx {
            4 | 7 => ch == '-',
            _ => ch.is_ascii_digit(),
        })
}

fn default_from_account(parser_account_id: &str, tx_type: &str) -> String {
    match tx_type {
        "income" => "EXTERNAL_PAYER".to_string(),
        "expense" => parser_account_id.to_string(),
        "internal_transfer" => parser_account_id.to_string(),
        _ => parser_account_id.to_string(),
    }
}

fn default_to_account(parser_account_id: &str, tx_type: &str) -> String {
    match tx_type {
        "income" => parser_account_id.to_string(),
        "expense" => "EXTERNAL_PAYEE".to_string(),
        "internal_transfer" => "EXTERNAL_PAYEE".to_string(),
        _ => "EXTERNAL_PAYEE".to_string(),
    }
}

fn make_txn_id(
    date: &str,
    from_account_id: &str,
    to_account_id: &str,
    amount: f64,
    currency: &str,
    description: &str,
    row_idx: usize,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(date.as_bytes());
    hasher.update(b"|");
    hasher.update(from_account_id.as_bytes());
    hasher.update(b"|");
    hasher.update(to_account_id.as_bytes());
    hasher.update(b"|");
    hasher.update(format!("{:.6}", amount).as_bytes());
    hasher.update(b"|");
    hasher.update(currency.as_bytes());
    hasher.update(b"|");
    hasher.update(description.as_bytes());
    hasher.update(b"|");
    hasher.update(row_idx.to_string().as_bytes());
    let digest = hasher.finalize();
    format!("GEN-{}", hex::encode(&digest[..12]))
}

fn uniquify_headers(headers: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashMap::<String, usize>::new();
    headers
        .into_iter()
        .map(|h| {
            let count = seen.entry(h.clone()).or_insert(0);
            *count += 1;
            if *count == 1 {
                h
            } else {
                format!("{}_{}", h, count)
            }
        })
        .collect()
}

fn normalize_text(input: &str) -> String {
    input
        .replace('\u{00A0}', " ")
        .replace('\r', " ")
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

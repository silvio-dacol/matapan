# Transaction Rule Engine

A flexible and scalable rule engine for automatically processing transactions in the Matapan financial application.

## Features

- ✅ **Flexible Conditions**: Match on description, amount, date, account, category, type, currency
- ✅ **Multiple Actions**: Set categories, tags, accounts, mark for review
- ✅ **Priority-based**: Rules execute in order based on priority
- ✅ **AND/OR Logic**: Combine conditions with AND or OR operators
- ✅ **RESTful API**: Full CRUD operations via HTTP endpoints
- ✅ **Test Mode**: Test rules before applying them
- ✅ **Batch Processing**: Apply rules to multiple transactions at once

## API Endpoints

### Rules Management

#### GET /api/rules
Returns all rules sorted by priority.

**Response:**
```json
[
  {
    "rule_id": "rent-categorization",
    "name": "Categorize Rent Payments",
    "description": "Auto-categorize rent transactions",
    "enabled": true,
    "priority": 1,
    "conditions": [
      {
        "type": "description_contains",
        "value": "RENT"
      }
    ],
    "condition_operator": "AND",
    "actions": [
      {
        "type": "set_category",
        "category": "housing"
      }
    ]
  }
]
```

#### GET /api/rules/:rule_id
Get a specific rule by ID.

#### POST /api/rules
Create a new rule.

**Request Body:**
```json
{
  "rule_id": "salary-rule",
  "name": "Categorize Salary",
  "description": "Identify salary payments",
  "enabled": true,
  "priority": 1,
  "conditions": [
    {
      "type": "description_contains",
      "value": "LÖN"
    },
    {
      "type": "type_equals",
      "value": "income"
    }
  ],
  "condition_operator": "AND",
  "actions": [
    {
      "type": "set_category",
      "category": "salary"
    }
  ]
}
```

#### PUT /api/rules/:rule_id
Update an existing rule.

#### DELETE /api/rules/:rule_id
Delete a rule.

#### POST /api/rules/:rule_id/toggle
Toggle a rule's enabled status.

#### POST /api/rules/reorder
Reorder rules by priority.

**Request Body:**
```json
{
  "rule_ids": ["rule1", "rule2", "rule3"]
}
```

### Rule Testing & Application

#### POST /api/rules/test
Test a rule against a sample transaction without saving.

**Request Body:**
```json
{
  "rule": {
    "rule_id": "test",
    "name": "Test Rule",
    "enabled": true,
    "priority": 1,
    "conditions": [
      {
        "type": "description_contains",
        "value": "RENT"
      }
    ],
    "condition_operator": "AND",
    "actions": [
      {
        "type": "set_category",
        "category": "housing"
      }
    ]
  },
  "transaction": {
    "description": "RENT payment",
    "category": "uncategorized",
    "amount": 8500.0
  }
}
```

**Response:**
```json
{
  "matches": true,
  "modified_transaction": {
    "description": "RENT payment",
    "category": "housing",
    "amount": 8500.0
  },
  "applied_actions": ["Set category to 'housing'"]
}
```

#### POST /api/rules/apply
Apply all enabled rules to a batch of transactions.

**Request Body:**
```json
{
  "transactions": [
    {
      "txn_id": "txn-1",
      "description": "RENT payment",
      "category": "uncategorized",
      "amount": 8500.0
    },
    {
      "txn_id": "txn-2",
      "description": "LÖN",
      "category": "uncategorized",
      "amount": 32307.0,
      "type": "income"
    }
  ]
}
```

**Response:**
```json
{
  "modified_transactions": [...],
  "results": [
    {
      "transaction_index": 0,
      "txn_id": "txn-1",
      "applied_actions": ["Set category to 'housing'"]
    },
    {
      "transaction_index": 1,
      "txn_id": "txn-2",
      "applied_actions": ["Set category to 'salary'"]
    }
  ]
}
```

## Condition Types

### Description Conditions
- `description_contains`: Check if description contains text (case-insensitive)
- `description_matches`: Match against regex pattern
- `description_equals`: Exact match (case-insensitive)

### Amount Conditions
- `amount_greater_than`: Amount > value
- `amount_less_than`: Amount < value
- `amount_equals`: Amount == value (with 0.01 tolerance)
- `amount_between`: min <= Amount <= max

### Category Conditions
- `category_equals`: Category matches exactly
- `category_in`: Category is in list

### Type Conditions
- `type_equals`: Transaction type (income/expense/transfer)

### Account Conditions
- `from_account_equals`: Match source account
- `to_account_equals`: Match destination account

### Date Conditions
- `date_before`: Date < value
- `date_after`: Date > value
- `date_between`: start <= Date <= end

### Currency Conditions
- `currency_equals`: Match currency code

### Custom Field Conditions
- `custom_field_contains`: Check custom field contains value
- `custom_field_equals`: Check custom field equals value

## Action Types

### Set Category
```json
{
  "type": "set_category",
  "category": "housing"
}
```

### Add Tag
```json
{
  "type": "add_tag",
  "tag": "recurring"
}
```

### Set Field
```json
{
  "type": "set_field",
  "field": "notes",
  "value": "Auto-categorized"
}
```

### Set Accounts
```json
{
  "type": "set_from_account",
  "account_id": "SEB_CHECKING"
}
```
```json
{
  "type": "set_to_account",
  "account_id": "EXTERNAL_PAYEE"
}
```

### Mark for Review
```json
{
  "type": "mark_for_review",
  "reason": "Large expense"
}
```

### Set Type
```json
{
  "type": "set_type",
  "transaction_type": "expense"
}
```

## Rule Examples

### 1. Categorize Rent
```json
{
  "rule_id": "rent-rule",
  "name": "Categorize Rent",
  "enabled": true,
  "priority": 1,
  "conditions": [
    {
      "type": "description_contains",
      "value": "RENT"
    }
  ],
  "condition_operator": "AND",
  "actions": [
    {
      "type": "set_category",
      "category": "housing"
    }
  ]
}
```

### 2. Categorize Food (Multiple Merchants)
```json
{
  "rule_id": "food-rule",
  "name": "Categorize Food",
  "enabled": true,
  "priority": 2,
  "conditions": [
    {
      "type": "description_contains",
      "value": "FOODMARKET"
    },
    {
      "type": "description_contains",
      "value": "HEMKÖPS"
    }
  ],
  "condition_operator": "OR",
  "actions": [
    {
      "type": "set_category",
      "category": "food"
    }
  ]
}
```

### 3. Flag Large Expenses
```json
{
  "rule_id": "large-expense",
  "name": "Flag Large Expenses",
  "enabled": true,
  "priority": 10,
  "conditions": [
    {
      "type": "amount_greater_than",
      "value": 10000.0
    },
    {
      "type": "type_equals",
      "value": "expense"
    }
  ],
  "condition_operator": "AND",
  "actions": [
    {
      "type": "mark_for_review",
      "reason": "Large expense - verify"
    },
    {
      "type": "add_tag",
      "tag": "large-expense"
    }
  ]
}
```

### 4. Categorize Salary
```json
{
  "rule_id": "salary-rule",
  "name": "Categorize Salary",
  "enabled": true,
  "priority": 1,
  "conditions": [
    {
      "type": "description_contains",
      "value": "LÖN"
    },
    {
      "type": "type_equals",
      "value": "income"
    }
  ],
  "condition_operator": "AND",
  "actions": [
    {
      "type": "set_category",
      "category": "salary"
    }
  ]
}
```

### 5. Apple Subscriptions
```json
{
  "rule_id": "apple-subscription",
  "name": "Apple Subscriptions",
  "enabled": true,
  "priority": 3,
  "conditions": [
    {
      "type": "description_contains",
      "value": "APPLE COM"
    },
    {
      "type": "amount_equals",
      "value": 249.0
    }
  ],
  "condition_operator": "AND",
  "actions": [
    {
      "type": "set_category",
      "category": "subscriptions"
    },
    {
      "type": "add_tag",
      "tag": "recurring"
    }
  ]
}
```

### 6. Västtrafik Public Transport
```json
{
  "rule_id": "transport-rule",
  "name": "Public Transport",
  "enabled": true,
  "priority": 2,
  "conditions": [
    {
      "type": "description_contains",
      "value": "VÄSTTRAFIK"
    }
  ],
  "condition_operator": "AND",
  "actions": [
    {
      "type": "set_category",
      "category": "transport"
    }
  ]
}
```

## Best Practices

1. **Priority Management**: Lower numbers execute first. Critical rules (e.g., salary) should have low priority numbers.

2. **Specific Before General**: Put specific rules (e.g., "APPLE COM/BI" for specific subscription) before general rules (e.g., "APPLE" for all Apple transactions).

3. **Test Rules**: Always use `/api/rules/test` endpoint before creating rules to verify behavior.

4. **Use OR for Variations**: When merchants appear with variations, use OR conditions:
   ```json
   "conditions": [
     {"type": "description_contains", "value": "FOODMARKET"},
     {"type": "description_contains", "value": "FOOD MARKET"},
     {"type": "description_contains", "value": "FOODMRKT"}
   ],
   "condition_operator": "OR"
   ```

5. **Review Flags**: Use `mark_for_review` for unusual patterns that need human verification.

6. **Incremental Implementation**: Start with high-volume, easy-to-categorize transactions (rent, salary, subscriptions), then expand.

## Integration with Transaction Import

When importing transactions from bank statements, you can:

1. Import raw transactions
2. Apply all rules via `/api/rules/apply`
3. Review flagged transactions
4. Save to database

This ensures consistent categorization across all transactions.

## Future Enhancements

- [ ] Rule templates library
- [ ] Machine learning suggestions for new rules
- [ ] Rule effectiveness analytics
- [ ] Scheduled rule application
- [ ] Rule conflicts detection
- [ ] Import/export rule sets

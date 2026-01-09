# Transaction Rule Engine

A lean and efficient rule engine for automatically processing transactions in the Matapan financial application.

## Features

- ✅ **Description Matching**: Match transactions by description text (contains, equals, regex)
- ✅ **Auto-Categorization**: Automatically set transaction categories
- ✅ **Account Routing**: Auto-assign from/to accounts
- ✅ **Type Classification**: Set transaction types (income/expense/transfer)
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

All conditions are based on transaction descriptions to keep the engine lean and focused.

### Description Conditions
- `description_contains`: Check if description contains text (case-insensitive)
- `description_matches`: Match against regex pattern for complex patterns
- `description_equals`: Exact match (case-insensitive)

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
Automatically categorize transactions based on their description.
```json
{
  "type": "set_category",
  "category": "housing"
}
```

### Set From Account
Set the source account for the transaction.
```json
{
  "type": "set_from_account",
  "account_id": "SEB_CHECKING"
}
```

### Set To Account
Set the destination account for the transaction.
```json
{
  "type": "set_to_account",
  "account_id": "EXTERNAL_PAYEE"
}
```

### Set Type
Set the transaction type (income, expense, or transfer).ory": "housing"
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

1. **Priority Management**: Lower numbers execute first. Critical rules (e.g., salary, rent) should have low priority numbers.

2. **Specific Before General**: Put specific rules before general rules. For example, "APPLE COM/BILL" for a specific subscription before "APPLE" for all Apple transactions.

3. **Test Rules**: Always use `/api/rules/test` endpoint before creating rules to verify behavior.

4. **Use OR for Merchant Variations**: When merchants appear with different spellings, use OR conditions:
   ```json
   "conditions": [
     {"type": "description_contains", "value": "FOODMARKET"},
     {"type": "description_contains", "value": "FOOD MARKET"},
     {"type": "description_contains", "value": "ICA"}
   ],
   "condition_operator": "OR"
   ```

5. **Use Regex for Complex Patterns**: When you need to match multiple variations or complex patterns, use `description_matches` with regex:
   ```json
   "conditions": [
     {"type": "description_matches", "pattern": "(NETFLIX|SPOTIFY|APPLE)"}
   ]
   ```

6. **Combine Multiple Actions**: You can set category, type, and accounts all in one rule for complete transaction automation.

7. **Incremental Implementation**: Start with high-volume, easy-to-categorize transactions (rent, salary, groceries), then expand.

## Integration with Transaction Import

When importing transactions from bank statements, you can:

1. Import raw transactions
2. Apply all rules via `/api/rules/apply`
3. Review auto-categorized transactions
4. Save to database

This ensures consistent categorization across all transactions.

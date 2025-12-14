# The Data Model

## The Sections of the Model

The data model is divided into four main sections that interact together to provide a clear picture of the user's financial situation. These sections will then be used to build the three main financial statements and the dashboard.

### Instruments

What the thing is in abstract. The economic object with its own rules. Cash, ETF, stock, fund, loan, mortgage, credit card. Defined once, reused everywhere.

### Accounts

Where the thing lives. The container provided by an institution. Bank account, brokerage account, pension account, credit card, mortgage contract.

### Positions

How much of an instrument you hold in a specific account at a specific date. Quantity plus valuation snapshot. This is what builds the balance sheet.

### Transactions

What changed over time. Movements of money or instruments that explain how positions evolved. Income, expenses, transfers, trades, loan payments.

### Month-end Snapshots

To capture the state of accounts and positions at the end of each month, month-end snapshots are created. These snapshots help in tracking and validating the other entries in the data model.

## The Bank Statement Parsers

To facilitate the data entry process, some bank statement parsers for common banks and brokers. These parsers extract relevant financial data from bank statements and format it according to our structured data model.

### How to Build a Parser

To build a bank statement parser, follow these steps:

1. **Understand the Bank Statement Format**: Obtain sample bank statements from the target bank or broker. Analyze the format, structure, and data fields present in the statements.

2. **Set Up the Development Environment**: Ensure you have Rust installed and set up a new crate for your parser within the `crates/data/bank_statement_parsers` directory.

3. **Implement the Parser Logic**: Write the code to read and parse the bank statement files. Extract relevant data fields such as account information, transaction details, balances, and dates.

4. **Map Data to the Data Model**: Convert the extracted data into the structured format defined by our data model. This includes creating instances of Instruments, Accounts, Positions, and Transactions as needed.

   - **Accounts** should be created new or linked to existing ones based on unique identifiers. If a new account is detected, then it can just be added automtically. If an account already exists, then it should be matched based on the unique identifier. Also here, the bank or broker statements may not provide full details, you might implement a post-check to allow the user to fill in missing information manually.
   - **Instruments** should be created or linked based on unique identifiers like ISIN, or ticker symbols. As the bank or broker statements may not provide full details, you might implement a post-check to allow the user to fill in missing information manually.
   - **Positions** should be created for each snapshot date found in the statements. Also in this case, some information may be missing, so a post-check to allow the user to fill in missing information manually might be needed.
   - **Transactions** should be created for each transaction found in the statements. Again, some information may be missing, so a post-check to allow the user to fill in missing information manually might be needed.
   - **Month-end Snapshots** should be created to capture the state of accounts and positions at the end of each month this is mostly manual.

## What Should Be Parsed Automatically (High Frequency)

### 1. Transactions

Parsed fields:

- date
- account_id
- amount
- currency
- description

### 2. Positions

Parsed fields:

- instrument_id
- quantity
- price
- gross_amount
- fees
- trade date
- currency

Trades update:

- Positions
- Unrealized and realized P&L
- Balance Sheet only

## What is Manual now but will become Automatic with APIs (Medium Frequency)

### 1. Month-End Snapshots

Used for:

- Reconciliation
- Validation
- Detecting missing transactions

Why manual:

- They are used for validation so it is critical they are correct
- Once the API is in place, these can be fetched automatically

## What Should Be Defined Manually (Low Frequency)

### 1. Instruments

Examples:

- Cash SEK
- VWCE ETF
- TSLA stock
- Mortgage loan
- Pension fund

Why manual:

- Instruments are semantic objects
- Banks name the same thing differently
- AI can help suggest, but humans decide

### 2. Accounts

Examples:

- SEB Checking
- Revolut Current
- IBKR Margin
- Mortgage Account

Why manual:

- Accounts are stable
- Needed for clean aggregation
- Needed to distinguish assets vs liabilities

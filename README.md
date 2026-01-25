# Matapan: The Financial Analyst for Expats

A minimal, opinionated net worth and financial analists built for expats.

#### The goal is simple:

Give a clear picture of net worth, cash flow, and investments without overengineering, while keeping the data fully user owned.

#### What was the Matapan?

The Matapan (or Grosso) was a silver coin used in the Republic of Venice during the late Middle Ages. It was known for its reliability and stability, making it a trusted currency for trade and commerce across different regions.

#### Why starting with expats?

- They usually have more complex financial situations.
- They often lack tailored financial tools.
- They poorly understand the fiscal incentives and trick of the Country they live in.
- They end up owning assets in multiple Countries and Currencies.

I am an Italian expat myself who worked and studied in 4 different Countries: Italy, Canada, Sweden, and China. I am building this to help people like me optimise their financial life abroad.

For now this app is the foundation for a future financial analyst platform tailored to expats.

It will always be an open source project and the data will always be yours. The plan for the future is to build paid services on top of it but keep the philosophy untouched.

Feel free to contribute with the data parsers for your banks and brokers! Also features are welcome as long as they respect the core principles.

Starting with common banks and brokers from Italy, Sweden, and China.

## What this is

- A monthly net worth tracking system
- A structured data model for assets, liabilities, and cash flows
- A tool designed to show users what they lose by not tracking properly.

This is NOT a budgeting app.  
This is NOT a bank replacement.  
This is a financial clarity tool.

## Core principles

- User owns the data
- Simple categories, no category explosion
- Monthly snapshots instead of noisy daily tracking
- Explicit separation between balance and performance

## Local AI (Ollama only)

Matapan is local-first: it never calls online model providers.

If/when AI features are enabled, they are powered by a local Ollama server running on your machine.
The AI client refuses to run if the configured URL is not local (e.g. it must be `localhost` / `127.0.0.1`).

Environment variables:

- `OLLAMA_BASE_URL` (default: `http://localhost:11434`)
- `OLLAMA_MODEL` (default: `llama3.2`)

Example:

```bash
set OLLAMA_BASE_URL=http://localhost:11434
set OLLAMA_MODEL=llama3.2
```

## Tracked categories

### Assets

- Cash accounts
- Investment accounts
- Pension accounts
- Personal assets

### Liabilities

- Loans
- Credit cards
- Mortgages
- Money owed to others

### Credits

- Money other people owe you

Credits are tracked explicitly and are not merged with cash.

## Manual today, automated tomorrow

Right now:

- From the bank statements or the salary slips, you can get the data in your dashboard through the parsers.

Later:

- Bank and Broker APIs
- Tax system integrations
- Country specific fiscal logic

The structure is designed so automation does not break historical data.

## Roadmap

- Solid net worth dashboard with insights on cash flow, income statements, and balance sheets.
- Fiscal guidance per Country: Where are you from? Where do you live? How to optimize your own situation?
- Plug and play legislation modules for each Country.
- Local AI models (Ollama) trained on your own data to give you tailored financial advice.
- AI digital twin of yourself so to take the most unbiased financial decisions in tough moments.

## Status

Early stage.

Contributions are welcome if they respect the core principles.

Financial Dashboard Backend Redesign Specification (Markdown)

1. Overview

This document defines the new backend architecture for the Financial Dashboard. It explains how the data should be structured, processed, and stored. The goal is to remove redundancy, simplify the model, eliminate ECLI, and rely only on clean inflation-adjusted metrics based on HICP.

The system is based on three layers:

settings.json – global configuration and constants.

template.json – raw monthly input.

dashboard.json – computed, normalized, and aggregated output.

3. Backend Logic

Below is the logic the backend should apply when transforming template.json into a snapshot inside dashboard.json.

3.1 Currency Normalization

All balances and amounts must be converted to the base currency.

balance_eur = balance \* fx_rates[currency]

3.2 Asset Aggregation

Group assets and liabilities based on the category definitions in settings.json.

3.3 Net Worth Calculation

net_worth = total_assets - total_liabilities

3.4 Cash Flow Aggregates

income = sum(positive cash flows)
expenses = sum(negative cash flows)
net_cash_flow = income - expenses
save_rate = income > 0 ? net_cash_flow / income : 0

3.5 Inflation-Adjusted Wealth

inflation_factor = hicp / settings.hicp.base_value
net_worth_real = net_worth / inflation_factor

3.6 Investment Performance (TWR)

Time-weighted return must exclude net inflows.

4. Removed Components

The following elements were removed entirely from the backend and data model:

ECLI

Cost-of-living indices

City-based adjustments

Basket of goods (optional)

Any monthly city comparison logic

5. Data Flow Summary

User provides raw monthly input using template.json.

Backend loads settings.json.

Backend normalizes and aggregates data.

Backend computes inflation, TWR, net worth, cash flows, and yearly stats.

Backend outputs the final dashboard.json.

6. Notes

All inflation logic relies solely on HICP.

dashboard.json must contain only derived values, never configuration.

settings.json is the single source of truth for categories and inflation baseline.

The system is ready for future migration to an encrypted database.

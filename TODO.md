# Performance Metrics TODO

Make the metrics available like on the IBKR app. So with Value (including income) and performance (excluding income). Add a chart showing how many big mac you could buy with your net worth over time.

## 1. Data Model Extension

Add a `performance` section to each monthly snapshot (or compute on-the-fly):

```jsonc
"performance": {
  "nominal_return": 0.0082,      // monthly % growth excluding external inflows
  "hicp_monthly": 0.0017,        // inflation rate for that month
  "real_return": 0.0065,         // nominal_return - hicp_monthly
  "twr_cumulative": 0.0691,      // time-weighted cumulative real performance
  "benchmark": "EU Inflation (HICP)"
}
```

Decision pending: store vs compute dynamically. Prefer compute if source data (net worth, inflows, hicp) is available.

## 2. Required Inputs

- Monthly net worth values (already present)
- Monthly external net cash flows (salary, deposits, withdrawals)
- Monthly HICP (already in normalization crate?)

## 3. Calculations

- Nominal monthly return (exclude external inflows):
  `(NW_t - NW_{t-1} - ExternalInflows_t) / NW_{t-1}`
- Real monthly return: `nominal_monthly - hicp_monthly`
- Time-weighted cumulative (TWR): chain (1 + real_monthly) across months.

## 4. Backend Implementation

- Add helpers in `data_pipeline` or a dedicated performance module.
- Inject computed `performance` section when serving snapshot(s) via API.
- Ensure no double counting of inflows; clarify inflow source.

## 5. API

- Extend snapshot DTO to include `performance`.
- Optionally add endpoint `/performance/series` for charting (value vs TWR).

## 6. Frontend

- New chart: "Value vs Performance" (two lines: net worth absolute vs cumulative TWR scaled to starting NW).
- Tooltip shows both nominal cumulative (%) and real cumulative (%) if desired.

## 7. Validation

- Reproduce example (Jan 2025 -> Oct 2025) using stored data:
  - Net worth Jan: 113,377
  - Net worth Oct: 208,174
  - External inflows (Jan-Oct): ~27,000
  - Inflation total: 2.1% => adjust monthly approximation.
  - Nominal growth excluding inflows ≈ 59.8%
  - Real ≈ 57.7%
- Cross-check against chained monthly returns.

## 8. Open Questions

- Source of external inflows: do we have granular data? If not, add placeholder.
- Store monthly HICP in snapshot vs recompute from normalization crate.

## 9. Next Steps (Tomorrow)

1. Confirm inflow data availability.
2. Decide persistent vs computed `performance` section.
3. Implement core calculation util + unit tests.
4. Extend API serializer.
5. Add frontend chart scaffold.

---

Short, intentional; we'll refine edge cases (zero previous month, missing HICP, large inflow anomalies) tomorrow.

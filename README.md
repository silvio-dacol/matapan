## Real Money Normalization Model

Goal: Convert nominal money into **real purchasing power**, adjusted for **inflation over time** and **cost of living across cities/countries**.

### Base Assumptions

- **Base currency:** EUR
- **Inflation index:** Euro Area HICP (monthly, Eurostat)
- **Cost of living index:** ECLI (Essential Cost of Living Index) from Numbeo
- **Purpose:** Compare income or net worth over time and across locations

### Essential Cost of Living Index (ECLI)

ECLI represents the essential cost of living in a city and the reference city is New York. So these are the translation in how much money you would need in New York for the same situation.

```
ECLI = 0.40·RentIndex + 0.35·GroceriesIndex + 0.25·CostOfLivingIndex
```

Normalize it:

```
ECLI_norm = ECLI / 100
```

### Inflation Adjustment (Deflator)

Choose a **base month** (e.g. Aug 2024 = 126.72).

The data comes from: https://ec.europa.eu/eurostat/databrowser/view/prc_hicp_midx__custom_18474914/default/table

```
Deflator(t) = HICP(base_month) / HICP(t)
```

### Final Normalization Formula

```
RealMoney(t) = Money_EUR(t) × Deflator(t) / ECLI_norm(city)
```

Where:

- `Money_EUR(t)` = salary, savings or net worth converted to EUR
- `Deflator(t)` = protects from inflation
- `ECLI_norm(city)` = adjusts for cost of living differences

### Interpretation

- If RealMoney increases → **you are actually richer**
- Works globally, even if you move country
- Keeps money comparable over time
- Useful for salary, savings, FIRE tracking, geo-arbitrage analysis

### Example (Gothenburg → Shanghai)

```
Money_EUR = 2920
Deflator = 0.966
ECLI(Gothenburg) = 49.8  → ECLI_norm = 0.498
```

Real purchasing power in Gothenburg:

```
RealMoney = 2920 × 0.966 / 0.498 = 5668 EUR
```

Real purchasing power in Shanghai (ECLI = 36.0 → 0.36):

```
RealMoney = 2920 × 0.966 / 0.36 = 7832 EUR
```

Moving could increases purchasing power!
